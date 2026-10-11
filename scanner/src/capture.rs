//! Screen region capture (GDI BitBlt) and game-window lookup.
//! The game process is never opened: it is found by name in a process snapshot and its window
//! matched by process ID, since GameGuard guards handles to it.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    SM_CXSCREEN, SM_CYSCREEN,
};
use windows::core::BOOL;

use crate::img::Img;

/// Without this, Windows display scaling above 100% hands us virtualised coordinates.
pub fn set_dpi_aware() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// The game window's client area in screen coordinates, and the process that owns it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameWindow {
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub w: usize,
    pub h: usize,
    pub source: Source,
}

/// How the game's position was found, for the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Window,     // its window, looked up just now
    Foreground, // the foreground window belongs to the game
    LastKnown,  // the window could not be looked up; where it was last seen
    MainScreen, // never seen; assumed fullscreen on the main screen
}

fn describe(hwnd: HWND, pid: u32, source: Source) -> Option<GameWindow> {
    unsafe {
        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc).ok()?;
        let mut pt = POINT { x: 0, y: 0 };
        if !ClientToScreen(hwnd, &mut pt).as_bool() {
            return None;
        }
        let (w, h) = ((rc.right - rc.left) as usize, (rc.bottom - rc.top) as usize);
        (w > 0 && h > 0).then_some(GameWindow { pid, x: pt.x, y: pt.y, w, h, source })
    }
}

/// IDs of running processes whose executable is `exe`, from a snapshot (no process is opened).
fn process_ids(exe: &str) -> Vec<u32> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                out.push(e.th32ProcessID);
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// Whether a process named `exe` is running (a snapshot, cheap enough to poll every few seconds).
/// Refreshes the process IDs the scans use.
pub fn running(exe: &str) -> bool {
    let pids = process_ids(exe);
    let up = !pids.is_empty();
    *PIDS.lock().unwrap_or_else(|e| e.into_inner()) = Some((exe.to_string(), pids, Instant::now()));
    up
}

// A snapshot of every process costs milliseconds on a busy machine (about 15 ms with 560 processes),
// too much for each scan, and the game's process ID does not change while it runs.
const PIDS_FOR: Duration = Duration::from_secs(2);
// Walking every window of the desktop is the other slow part; while the game's window is not known,
// it is done at most this often, and the fallbacks of `locate` cover the time in between.
const SEARCH_EVERY: Duration = Duration::from_secs(1);

static PIDS: std::sync::Mutex<Option<(String, Vec<u32>, Instant)>> = std::sync::Mutex::new(None); // (exe, pids, when)
static LAST_SEARCH: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
static GAME_WINDOW: std::sync::Mutex<Option<(isize, u32)>> = std::sync::Mutex::new(None); // (hwnd, pid)
static LAST_SEEN: std::sync::Mutex<Option<GameWindow>> = std::sync::Mutex::new(None);

/// The process IDs of `exe`, from a snapshot at most PIDS_FOR old.
fn game_pids(exe: &str) -> Vec<u32> {
    let mut cache = PIDS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((name, pids, at)) = &*cache
        && name == exe
        && at.elapsed() < PIDS_FOR
    {
        return pids.clone();
    }
    let pids = process_ids(exe);
    *cache = Some((exe.to_string(), pids.clone(), Instant::now()));
    pids
}

/// Where to capture the game. Inside the Stream Deck plugin, Windows sometimes stops listing the
/// game's windows to us a few seconds in (likely its anti-cheat), so this falls back to the
/// foreground window, then the last known position, then the main screen, for as long as the
/// game process runs. Capturing needs a position, not a window handle.
pub fn locate(exe: &str) -> Option<GameWindow> {
    if let Some(g) = find_window(exe) {
        *LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
        return Some(g);
    }
    let pids = game_pids(exe);
    let pid = *pids.first()?;
    if let Some(hwnd) = game_foreground(&pids)
        && let Some(g) = describe(hwnd, pid, Source::Foreground)
    {
        *LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(g);
        return Some(g);
    }
    if let Some(g) = *LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()) {
        return Some(GameWindow { pid, source: Source::LastKnown, ..g });
    }
    let (w, h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    (w > 0 && h > 0).then_some(GameWindow { pid, x: 0, y: 0, w: w as usize, h: h as usize, source: Source::MainScreen })
}

/// Why the game window cannot be used right now, for the log.
pub fn diagnose(exe: &str) -> String {
    let pids = process_ids(exe);
    if pids.is_empty() {
        return format!("no process named {exe}");
    }
    struct Seen {
        pids: Vec<u32>,
        windows: Vec<String>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lp: LPARAM) -> BOOL {
        let s = unsafe { &mut *(lp.0 as *mut Seen) };
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if s.pids.contains(&pid) {
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                s.windows.push(format!(
                    "visible={} minimised={} client={}x{}",
                    IsWindowVisible(hwnd).as_bool(),
                    IsIconic(hwnd).as_bool(),
                    rc.right - rc.left,
                    rc.bottom - rc.top
                ));
            }
        }
        BOOL(1)
    }
    let mut seen = Seen { pids: pids.clone(), windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut seen as *mut Seen as isize));
    }
    format!("pids {pids:?}, windows [{}]", seen.windows.join("; "))
}

/// The largest visible, non-minimised top-level window of the process `exe`, focused or not.
/// Its handle is cached; processes and windows are only looked up again when that window goes away,
/// and then at most every SEARCH_EVERY.
pub fn find_window(exe: &str) -> Option<GameWindow> {
    let usable = |hwnd: HWND| unsafe { IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
    let mut cache = GAME_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((h, pid)) = *cache {
        let hwnd = HWND(h as *mut _);
        let mut owner = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        // owner 0: Windows hides who owns the game's window at times (see game_foreground), but a handle
        // reused by another program would report that program, so the window is still the game's
        if (owner == pid || owner == 0) && usable(hwnd) {
            return describe(hwnd, pid, Source::Window);
        }
        *cache = None;
    }
    {
        let mut last = LAST_SEARCH.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < SEARCH_EVERY) {
            return None;
        }
        *last = Some(Instant::now());
    }

    struct Search {
        pids: Vec<u32>,
        best: Option<(isize, u32, i64)>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lp: LPARAM) -> BOOL {
        let s = unsafe { &mut *(lp.0 as *mut Search) };
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if !s.pids.contains(&pid) || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return BOOL(1);
            }
            let mut rc = RECT::default();
            if GetClientRect(hwnd, &mut rc).is_ok() {
                let area = (rc.right - rc.left) as i64 * (rc.bottom - rc.top) as i64;
                if area > s.best.map_or(0, |b| b.2) {
                    s.best = Some((hwnd.0 as isize, pid, area));
                }
            }
        }
        BOOL(1)
    }
    let pids = game_pids(exe);
    if pids.is_empty() {
        return None;
    }
    let mut search = Search { pids, best: None };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
    }
    let (h, pid, _) = search.best?;
    *cache = Some((h, pid));
    describe(HWND(h as *mut _), pid, Source::Window)
}

/// Who owns the foreground window right now, for the log: (pid, executable name from a snapshot).
pub fn foreground_owner() -> (u32, String) {
    let mut owner = 0u32;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return (0, "no foreground window".into());
        }
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
    }
    let mut name = String::from("?");
    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut more = Process32FirstW(snap, &mut e).is_ok();
            while more {
                if e.th32ProcessID == owner {
                    let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                    name = String::from_utf16_lossy(&e.szExeFile[..len]);
                    break;
                }
                more = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
    }
    (owner, name)
}

/// The foreground window, if it is the game's. Windows sometimes refuses to tell us who owns the
/// game's window (it reports process 0: the anti-cheat shields it), while ordinary windows always
/// report their owner; so an unidentifiable foreground window while the game runs is the game.
fn game_foreground(pids: &[u32]) -> Option<HWND> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        let known = GAME_WINDOW.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|(h, _)| h == hwnd.0 as isize);
        (pids.contains(&owner) || known || owner == 0).then_some(hwnd)
    }
}

/// Whether the game is the window receiving keyboard input.
pub fn game_in_focus(exe: &str) -> bool {
    let pids = game_pids(exe);
    !pids.is_empty() && game_foreground(&pids).is_some()
}

/// Keeps its GDI objects between grabs; most of a grab's cost is a fixed ~3 ms of BitBlt.
pub struct Grabber {
    screen: HDC,
    mem: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    w: i32,
    h: i32,
    buf: Vec<u8>,
}

// The DCs are only ever used from the thread that owns the Grabber.
unsafe impl Send for Grabber {}

impl Default for Grabber {
    fn default() -> Self {
        Self::new()
    }
}

impl Grabber {
    pub fn new() -> Grabber {
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            Grabber { screen, mem, bmp: HBITMAP::default(), old: HGDIOBJ::default(), w: 0, h: 0, buf: Vec::new() }
        }
    }

    pub fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Result<Img, String> {
        if w <= 0 || h <= 0 {
            return Err("empty region".into());
        }
        unsafe {
            if (w, h) != (self.w, self.h) {
                let bmp = CreateCompatibleBitmap(self.screen, w, h);
                let prev = SelectObject(self.mem, bmp.into());
                if self.old.is_invalid() {
                    self.old = prev;
                } else {
                    let _ = DeleteObject(self.bmp.into());
                }
                self.bmp = bmp;
                (self.w, self.h) = (w, h);
                self.buf = vec![0u8; (w * h * 4) as usize];
            }
            BitBlt(self.mem, 0, 0, w, h, Some(self.screen), x, y, SRCCOPY).map_err(|e| format!("BitBlt: {e}"))?;
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let lines = GetDIBits(self.mem, self.bmp, 0, h as u32, Some(self.buf.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            if lines != h {
                return Err("GetDIBits failed".into());
            }
        }
        Ok(Img::from_bgra(&self.buf, w as usize, h as usize))
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        unsafe {
            if !self.old.is_invalid() {
                SelectObject(self.mem, self.old);
                let _ = DeleteObject(self.bmp.into());
            }
            let _ = DeleteDC(self.mem);
            ReleaseDC(None, self.screen);
        }
    }
}
