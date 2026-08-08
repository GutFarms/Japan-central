#pragma once

// Optional local overrides. Copy from secrets.h.example if needed.
#if __has_include("secrets.h")
#include "secrets.h"
#endif

#ifndef WIFI_SSID
#define WIFI_SSID "YOUR_WIFI_SSID"
#endif

#ifndef WIFI_PASSWORD
#define WIFI_PASSWORD "YOUR_WIFI_PASSWORD"
#endif

#ifndef METRICS_UDP_PORT
#define METRICS_UDP_PORT 4210
#endif

#ifndef METRICS_STALE_MS
#define METRICS_STALE_MS 3000
#endif

// Wait this long after boot before sniffing / reconnecting Wi-Fi.
#ifndef WIFI_BOOT_DELAY_MS
#define WIFI_BOOT_DELAY_MS 5000
#endif

// How often to rescan when disconnected.
#ifndef WIFI_RETRY_MS
#define WIFI_RETRY_MS 8000
#endif
