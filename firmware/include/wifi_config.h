#pragma once

// Optional local overrides (gitignored). Copy from secrets.h.example.
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

// How long without a packet before the UI shows "waiting for host".
#ifndef METRICS_STALE_MS
#define METRICS_STALE_MS 3000
#endif
