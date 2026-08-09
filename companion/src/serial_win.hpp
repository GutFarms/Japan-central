#pragma once
#include <string>
#include <vector>
#include <windows.h>

struct SerialPort {
  HANDLE handle = INVALID_HANDLE_VALUE;
  std::wstring name;

  bool open(const std::wstring& portName, DWORD baud = 115200);
  void close();
  bool isOpen() const { return handle != INVALID_HANDLE_VALUE; }
  bool writeAll(const std::string& data);
  std::string readAvailable();
};

std::vector<std::wstring> listComPorts();
