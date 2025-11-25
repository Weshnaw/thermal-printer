Project to connect an esp32 to a small thermal printer through a uart serial interface, and then allow either a web endpoint or a mqtt server to send text print requests.

required env variables:
- MQTT_USER
- MQTT_PASSWORD
- WIFI_SSID
- WIFI_PASSWORD


Tested with Thermal Printer Model:
- MC206H

Inspiration:
- [scribe](https://github.com/UrbanCircles/scribe/tree/main)

TODO:
- I would like to implement some unit testing apparatus
- Update glue abstraction to do all peripheral initialization logic
- refactor the multi-core to be more clear and concise 
- setup a configuration that can be changed dynamically via mqtt
  - such as the IP address of mqtt
- update which mqtt crate used to have async as first class
- a method to calibrate power status ADC automatically
- add some way to allow start up in a degraded form / attempt a retry instead of panicing for some errors