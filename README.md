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
- CURRENT: refactor the code to be more readable
  - use a glue abstraction to clear device specific code from the more generic code 
- I would like to implement some unit testing apparatus
- make the distinction between thread 1 and thread 2 more clear / obvious
- setup a configuration that can be changed dynamically via mqtt
  - such as the IP address of mqtt
- refactor the multi-core to be more clear and concise 
- update which mqtt crate used to have async as first class