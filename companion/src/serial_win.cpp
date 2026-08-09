#include "serial_win.hpp"
#include <setupapi.h>
#include <devguid.h>
#include <cstring>

bool SerialPort::open(const std::wstring& portName, DWORD baud) {
  close();
  std::wstring path = portName;
  if (path.rfind(L"\\\\.\\", 0) != 0) {
    path = L"\\\\.\\" + portName;
  }
  handle = CreateFileW(path.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING,
                       FILE_ATTRIBUTE_NORMAL, nullptr);
  if (handle == INVALID_HANDLE_VALUE) return false;

  SetupComm(handle, 8192, 8192);
  DCB dcb{};
  dcb.DCBlength = sizeof(dcb);
  if (!GetCommState(handle, &dcb)) {
    close();
    return false;
  }
  dcb.BaudRate = baud;
  dcb.ByteSize = 8;
  dcb.Parity = NOPARITY;
  dcb.StopBits = ONESTOPBIT;
  dcb.fBinary = TRUE;
  dcb.fDtrControl = DTR_CONTROL_ENABLE;
  dcb.fRtsControl = RTS_CONTROL_ENABLE;
  if (!SetCommState(handle, &dcb)) {
    close();
    return false;
  }

  COMMTIMEOUTS to{};
  to.ReadIntervalTimeout = MAXDWORD;
  to.ReadTotalTimeoutMultiplier = 0;
  to.ReadTotalTimeoutConstant = 0;
  to.WriteTotalTimeoutConstant = 1000;
  SetCommTimeouts(handle, &to);
  PurgeComm(handle, PURGE_RXCLEAR | PURGE_TXCLEAR);
  name = portName;
  return true;
}

void SerialPort::close() {
  if (handle != INVALID_HANDLE_VALUE) {
    CloseHandle(handle);
    handle = INVALID_HANDLE_VALUE;
  }
  name.clear();
}

bool SerialPort::writeAll(const std::string& data) {
  if (!isOpen()) return false;
  DWORD written = 0;
  size_t off = 0;
  while (off < data.size()) {
    if (!WriteFile(handle, data.data() + off, (DWORD)(data.size() - off), &written, nullptr)) {
      return false;
    }
    off += written;
  }
  FlushFileBuffers(handle);
  return true;
}

std::string SerialPort::readAvailable() {
  std::string out;
  if (!isOpen()) return out;
  DWORD errors = 0;
  COMSTAT st{};
  ClearCommError(handle, &errors, &st);
  DWORD avail = st.cbInQue;
  if (avail == 0) return out;
  if (avail > 8192) avail = 8192;
  out.resize(avail);
  DWORD got = 0;
  if (!ReadFile(handle, out.data(), avail, &got, nullptr)) {
    out.clear();
    return out;
  }
  out.resize(got);
  return out;
}

std::vector<std::wstring> listComPorts() {
  std::vector<std::wstring> ports;
  // Query registry for COM ports
  HKEY key = nullptr;
  if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, L"HARDWARE\\DEVICEMAP\\SERIALCOMM", 0, KEY_READ, &key) !=
      ERROR_SUCCESS) {
    return ports;
  }
  for (DWORD i = 0;; i++) {
    wchar_t valueName[256];
    wchar_t data[256];
    DWORD valueLen = 256;
    DWORD dataLen = sizeof(data);
    DWORD type = 0;
    LONG rc = RegEnumValueW(key, i, valueName, &valueLen, nullptr, &type, (LPBYTE)data, &dataLen);
    if (rc == ERROR_NO_MORE_ITEMS) break;
    if (rc != ERROR_SUCCESS) continue;
    if (type == REG_SZ) ports.emplace_back(data);
  }
  RegCloseKey(key);
  return ports;
}
