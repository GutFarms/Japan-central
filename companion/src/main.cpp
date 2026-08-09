#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <string>
#include <vector>
#include <cstdio>
#include <sstream>
#include "serial_win.hpp"
#include "protocol.hpp"

enum Ids : int {
  ID_PORT = 1001,
  ID_REFRESH,
  ID_CONNECT,
  ID_DISCONNECT,
  ID_STATUS,
  ID_SSID,
  ID_WIFI_PASS,
  ID_STRATUM,
  ID_WORKER,
  ID_POOL_PASS,
  ID_AUTH,
  ID_CPU,
  ID_HASH_FOCUS,
  ID_SAVE_REBOOT,
  ID_RECONNECT,
  ID_APPLY_CLOCK,
  ID_TIMER = 2001,
};

static SerialPort g_port;
static std::string g_rx;
static BoardStatus g_status;
static BoardConfig g_config;
static HWND g_hwnd;
static HFONT g_font;
static HBRUSH g_bgBrush;
static HBRUSH g_panelBrush;

static COLORREF COL_BG = RGB(8, 12, 16);
static COLORREF COL_PANEL = RGB(18, 26, 36);
static COLORREF COL_TEXT = RGB(228, 238, 248);

static std::wstring toWide(const std::string& s) {
  if (s.empty()) return L"";
  int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0);
  std::wstring w(n, 0);
  MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), w.data(), n);
  return w;
}

static std::string toUtf8(const std::wstring& w) {
  if (w.empty()) return {};
  int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), nullptr, 0, nullptr, nullptr);
  std::string s(n, 0);
  WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), s.data(), n, nullptr, nullptr);
  return s;
}

static std::wstring getText(HWND h, int id) {
  HWND c = GetDlgItem(h, id);
  int n = GetWindowTextLengthW(c);
  std::wstring w(n, 0);
  GetWindowTextW(c, w.data(), n + 1);
  return w;
}

static void setText(HWND h, int id, const std::wstring& w) {
  SetWindowTextW(GetDlgItem(h, id), w.c_str());
}

static void setStatusLine(HWND h, const std::wstring& w) {
  setText(h, ID_STATUS, w);
}

static void refreshPorts(HWND h) {
  HWND cb = GetDlgItem(h, ID_PORT);
  SendMessageW(cb, CB_RESETCONTENT, 0, 0);
  auto ports = listComPorts();
  for (auto& p : ports) SendMessageW(cb, CB_ADDSTRING, 0, (LPARAM)p.c_str());
  if (!ports.empty()) SendMessageW(cb, CB_SETCURSEL, 0, 0);
}

static void paintStats(HWND h) {
  wchar_t buf[512];
  swprintf(buf, 512,
           L"H/s  %.2f\r\n"
           L"Pool  %s\r\n"
           L"Accepted %u   Rejected %u\r\n"
           L"WiFi %s   IP %s\r\n"
           L"CPU %u MHz   Focus %s\r\n"
           L"Worker %s\r\n"
           L"Stratum %s\r\n"
           L"FW %s   Configured %s",
           g_status.hashrateHs, toWide(g_status.connected ? "CONNECTED" : g_status.pool).c_str(),
           g_status.accepted, g_status.rejected, toWide(g_status.wifi).c_str(),
           toWide(g_status.ip).c_str(), g_status.cpuMhz, g_status.hashFocus ? L"on" : L"off",
           toWide(g_status.address.empty() ? g_config.worker : g_status.address).c_str(),
           toWide(g_status.stratum.empty() ? g_config.stratum : g_status.stratum).c_str(),
           toWide(g_config.fw).c_str(), g_config.configured ? L"yes" : L"no");
  setText(h, ID_STATUS, buf);
}

static void pollBoard(HWND h) {
  if (!g_port.isOpen()) return;
  if (auto line = usbCmd(g_port, g_rx, "cmp status", 3500)) {
    parseCmpStatus(*line, g_status);
  }
  if (auto line = usbCmd(g_port, g_rx, "cmp config", 3500)) {
    parseCmpConfig(*line, g_config);
    if (!g_config.wifiSsid.empty() && getText(h, ID_SSID).empty())
      setText(h, ID_SSID, toWide(g_config.wifiSsid));
    if (!g_config.stratum.empty() && getText(h, ID_STRATUM).empty())
      setText(h, ID_STRATUM, toWide(g_config.stratum));
    if (!g_config.worker.empty() && getText(h, ID_WORKER).empty())
      setText(h, ID_WORKER, toWide(g_config.worker));
    wchar_t mhz[16];
    swprintf(mhz, 16, L"%u", g_config.cpuMhz);
    setText(h, ID_CPU, mhz);
    SendMessageW(GetDlgItem(h, ID_HASH_FOCUS), BM_SETCHECK,
                 g_config.hashFocus ? BST_CHECKED : BST_UNCHECKED, 0);
  }
  paintStats(h);
}

static HWND addLabel(HWND parent, const wchar_t* text, int x, int y, int w, int h) {
  HWND c = CreateWindowW(L"STATIC", text, WS_CHILD | WS_VISIBLE, x, y, w, h, parent, nullptr,
                         GetModuleHandleW(nullptr), nullptr);
  SendMessageW(c, WM_SETFONT, (WPARAM)g_font, TRUE);
  return c;
}

static HWND addEdit(HWND parent, int id, int x, int y, int w, int h, bool password = false) {
  DWORD style = WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL;
  if (password) style |= ES_PASSWORD;
  HWND c = CreateWindowW(L"EDIT", L"", style, x, y, w, h, parent, (HMENU)(intptr_t)id,
                         GetModuleHandleW(nullptr), nullptr);
  SendMessageW(c, WM_SETFONT, (WPARAM)g_font, TRUE);
  return c;
}

static HWND addBtn(HWND parent, int id, const wchar_t* text, int x, int y, int w, int h) {
  HWND c = CreateWindowW(L"BUTTON", text, WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, x, y, w, h, parent,
                         (HMENU)(intptr_t)id, GetModuleHandleW(nullptr), nullptr);
  SendMessageW(c, WM_SETFONT, (WPARAM)g_font, TRUE);
  return c;
}

static void createUi(HWND h) {
  int y = 16;
  addLabel(h, L"CYD Companion", 20, y, 300, 28);
  y += 36;
  addLabel(h, L"COM port", 20, y + 4, 80, 20);
  CreateWindowW(L"COMBOBOX", L"", WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST | WS_VSCROLL, 100, y,
                160, 200, h, (HMENU)ID_PORT, GetModuleHandleW(nullptr), nullptr);
  SendMessageW(GetDlgItem(h, ID_PORT), WM_SETFONT, (WPARAM)g_font, TRUE);
  addBtn(h, ID_REFRESH, L"Refresh", 270, y, 90, 28);
  addBtn(h, ID_CONNECT, L"Connect", 370, y, 100, 28);
  addBtn(h, ID_DISCONNECT, L"Disconnect", 480, y, 110, 28);
  y += 44;

  addLabel(h, L"Live stats (board)", 20, y, 240, 20);
  y += 24;
  HWND st = CreateWindowW(L"EDIT", L"Connect USB to see miner stats.",
                          WS_CHILD | WS_VISIBLE | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL,
                          20, y, 570, 150, h, (HMENU)ID_STATUS, GetModuleHandleW(nullptr), nullptr);
  SendMessageW(st, WM_SETFONT, (WPARAM)g_font, TRUE);
  y += 170;

  addLabel(h, L"Control — WiFi / Pool / Clock (app is the only setup UI)", 20, y, 500, 20);
  y += 28;
  addLabel(h, L"WiFi SSID", 20, y + 4, 100, 20);
  addEdit(h, ID_SSID, 130, y, 200, 26);
  addLabel(h, L"WiFi pass", 350, y + 4, 80, 20);
  addEdit(h, ID_WIFI_PASS, 430, y, 160, 26, true);
  y += 36;
  addLabel(h, L"Stratum", 20, y + 4, 100, 20);
  addEdit(h, ID_STRATUM, 130, y, 460, 26);
  y += 36;
  addLabel(h, L"Worker", 20, y + 4, 100, 20);
  addEdit(h, ID_WORKER, 130, y, 460, 26);
  y += 36;
  addLabel(h, L"Pool pass", 20, y + 4, 100, 20);
  addEdit(h, ID_POOL_PASS, 130, y, 200, 26, true);
  addLabel(h, L"Auth", 350, y + 4, 50, 20);
  addEdit(h, ID_AUTH, 400, y, 190, 26, true);
  y += 36;
  addLabel(h, L"CPU MHz", 20, y + 4, 100, 20);
  addEdit(h, ID_CPU, 130, y, 80, 26);
  setText(h, ID_CPU, L"240");
  HWND chk = CreateWindowW(L"BUTTON", L"Hash focus", WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX, 230, y,
                           120, 26, h, (HMENU)ID_HASH_FOCUS, GetModuleHandleW(nullptr), nullptr);
  SendMessageW(chk, WM_SETFONT, (WPARAM)g_font, TRUE);
  SendMessageW(chk, BM_SETCHECK, BST_CHECKED, 0);
  y += 40;
  addBtn(h, ID_SAVE_REBOOT, L"Save & reboot", 20, y, 140, 34);
  addBtn(h, ID_APPLY_CLOCK, L"Apply clock", 170, y, 120, 34);
  addBtn(h, ID_RECONNECT, L"Reconnect pool", 300, y, 140, 34);

  setText(h, ID_STRATUM, L"stratum+tcp://ltc.viabtc.io:3333");
  refreshPorts(h);
}

static std::string buildSetBody(HWND h, bool includeWifiPass, bool includePoolPass) {
  std::ostringstream body;
  auto add = [&](const char* k, const std::wstring& v) {
    if (v.empty()) return;
    if (body.tellp() > 0) body << '&';
    body << k << '=' << urlEncode(toUtf8(v));
  };
  add("wifi_ssid", getText(h, ID_SSID));
  if (includeWifiPass) add("wifi_password", getText(h, ID_WIFI_PASS));
  add("stratum", getText(h, ID_STRATUM));
  add("worker", getText(h, ID_WORKER));
  if (includePoolPass) add("password", getText(h, ID_POOL_PASS));
  add("cpu_mhz", getText(h, ID_CPU));
  bool focus = SendMessageW(GetDlgItem(h, ID_HASH_FOCUS), BM_GETCHECK, 0, 0) == BST_CHECKED;
  if (body.tellp() > 0) body << '&';
  body << "hash_focus=" << (focus ? "true" : "false");
  auto auth = getText(h, ID_AUTH);
  if (auth.empty()) auth = getText(h, ID_POOL_PASS);
  if (!auth.empty()) {
    body << "&auth=" << urlEncode(toUtf8(auth));
  }
  body << "&reboot=true";
  return body.str();
}

static void onSaveReboot(HWND h) {
  if (!g_port.isOpen()) {
    MessageBoxW(h, L"Connect USB first.", L"CYD Companion", MB_ICONWARNING);
    return;
  }
  std::string body = buildSetBody(h, true, true);
  std::string cmd = "cmp set " + body;
  auto r = usbCmd(g_port, g_rx, cmd, 5000);
  if (!r) {
    MessageBoxW(h, L"Save failed (CMPERR / timeout). Check auth / fields.", L"CYD Companion",
                MB_ICONERROR);
    return;
  }
  MessageBoxW(h, L"Saved. Board is rebooting — reconnect in a few seconds.", L"CYD Companion",
              MB_OK);
}

static void onApplyClock(HWND h) {
  if (!g_port.isOpen()) return;
  auto auth = getText(h, ID_AUTH);
  if (auth.empty()) auth = getText(h, ID_POOL_PASS);
  std::ostringstream body;
  body << "cpu_mhz=" << urlEncode(toUtf8(getText(h, ID_CPU)));
  bool focus = SendMessageW(GetDlgItem(h, ID_HASH_FOCUS), BM_GETCHECK, 0, 0) == BST_CHECKED;
  body << "&hash_focus=" << (focus ? "true" : "false");
  if (!auth.empty()) body << "&auth=" << urlEncode(toUtf8(auth));
  body << "&reboot=true";
  auto r = usbCmd(g_port, g_rx, "cmp clock " + body.str(), 5000);
  if (!r) MessageBoxW(h, L"Clock apply failed.", L"CYD Companion", MB_ICONERROR);
  else MessageBoxW(h, L"Clock queued — board rebooting.", L"CYD Companion", MB_OK);
}

static void onReconnect(HWND h) {
  if (!g_port.isOpen()) return;
  auto auth = getText(h, ID_AUTH);
  if (auth.empty()) auth = getText(h, ID_POOL_PASS);
  std::string cmd = "cmp set reconnect=true";
  if (!auth.empty()) cmd += "&auth=" + urlEncode(toUtf8(auth));
  auto r = usbCmd(g_port, g_rx, cmd, 4000);
  if (!r) MessageBoxW(h, L"Reconnect failed.", L"CYD Companion", MB_ICONERROR);
}

static LRESULT CALLBACK WndProc(HWND h, UINT msg, WPARAM wParam, LPARAM lParam) {
  switch (msg) {
    case WM_CREATE:
      createUi(h);
      SetTimer(h, ID_TIMER, 2500, nullptr);
      return 0;
    case WM_CTLCOLORSTATIC:
    case WM_CTLCOLOREDIT: {
      HDC hdc = (HDC)wParam;
      SetTextColor(hdc, COL_TEXT);
      SetBkColor(hdc, COL_PANEL);
      return (LRESULT)g_panelBrush;
    }
    case WM_ERASEBKGND: {
      RECT rc;
      GetClientRect(h, &rc);
      FillRect((HDC)wParam, &rc, g_bgBrush);
      return 1;
    }
    case WM_COMMAND: {
      int id = LOWORD(wParam);
      if (id == ID_REFRESH) refreshPorts(h);
      else if (id == ID_CONNECT) {
        wchar_t port[64]{};
        GetWindowTextW(GetDlgItem(h, ID_PORT), port, 64);
        if (!port[0]) {
          MessageBoxW(h, L"Pick a COM port.", L"CYD Companion", MB_ICONWARNING);
          break;
        }
        if (!g_port.open(port)) {
          MessageBoxW(h, L"Could not open port. Close other serial apps.", L"CYD Companion",
                      MB_ICONERROR);
          break;
        }
        g_rx.clear();
        g_port.writeAll("\r\ncmp ping\r\n");
        Sleep(200);
        auto pong = usbCmd(g_port, g_rx, "cmp ping", 3500);
        setStatusLine(h, pong ? L"Connected — polling board…" : L"Port open (waiting for board)…");
        pollBoard(h);
      } else if (id == ID_DISCONNECT) {
        g_port.close();
        g_rx.clear();
        setStatusLine(h, L"Disconnected.");
      } else if (id == ID_SAVE_REBOOT) onSaveReboot(h);
      else if (id == ID_APPLY_CLOCK) onApplyClock(h);
      else if (id == ID_RECONNECT) onReconnect(h);
      return 0;
    }
    case WM_TIMER:
      if (wParam == ID_TIMER && g_port.isOpen()) pollBoard(h);
      return 0;
    case WM_DESTROY:
      KillTimer(h, ID_TIMER);
      g_port.close();
      PostQuitMessage(0);
      return 0;
  }
  return DefWindowProcW(h, msg, wParam, lParam);
}

int WINAPI wWinMain(HINSTANCE hi, HINSTANCE, PWSTR, int show) {
  INITCOMMONCONTROLSEX icc{sizeof(icc), ICC_STANDARD_CLASSES};
  InitCommonControlsEx(&icc);

  g_bgBrush = CreateSolidBrush(COL_BG);
  g_panelBrush = CreateSolidBrush(COL_PANEL);
  g_font = CreateFontW(18, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                       CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH | FF_SWISS, L"Segoe UI");

  WNDCLASSW wc{};
  wc.lpfnWndProc = WndProc;
  wc.hInstance = hi;
  wc.lpszClassName = L"CydCompanionWnd";
  wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
  wc.hbrBackground = g_bgBrush;
  RegisterClassW(&wc);

  g_hwnd = CreateWindowW(L"CydCompanionWnd", L"CYD Companion — miner control",
                         WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX, CW_USEDEFAULT,
                         CW_USEDEFAULT, 640, 720, nullptr, nullptr, hi, nullptr);
  ShowWindow(g_hwnd, show);
  UpdateWindow(g_hwnd);

  MSG msg;
  while (GetMessageW(&msg, nullptr, 0, 0)) {
    TranslateMessage(&msg);
    DispatchMessageW(&msg);
  }
  DeleteObject(g_font);
  DeleteObject(g_bgBrush);
  DeleteObject(g_panelBrush);
  return (int)msg.wParam;
}
