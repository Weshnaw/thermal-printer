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
- refactor the code to be more readable
- make the distinction between thread 1 and thread 2 more clear / obvious