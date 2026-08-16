Import("env")
# Silence Arduino core chatter on UART0 — companion owns the link.
env.Append(CPPDEFINES=[("ARDUINO_USB_CDC_ON_BOOT", 0)])
