# Přípojení zařízení Maschine k telefonu přes USB OTG nebo Bluetooth: Komplexní průvodce integrace s ESP32 a LED systémem

Vaše myšlenka na rozšíření současného setup s připojením jednoho zařízení Maschine k telefonu představuje zajímavou evoluci současných možností. Tento přístup nabízí značné možnosti integrace a může skutečně výrazně rozšířit funkčnost vašeho hudebního systému.

## Připojení Maschine k telefonu

### USB OTG připojení

Připojení Maschine MK2 nebo MK3 k telefonu přes USB OTG je technicky možné, ale má svá specifika. Android podporuje USB MIDI od verze 6.0 (API 23), což umožňuje přímé připojení MIDI zařízení [^4_1]. Důležité je, že Maschine zařízení musí být v MIDI režimu, aby mohla komunikovat bez své vlastní softwarové vrstvy [^4_2][^4_3].

Pro aktivaci MIDI režimu na Maschine MK2/MK3 je potřeba podržet **Shift + Channel (MIDI)** tlačítko [^4_4][^4_5]. Tento režim umožňuje zařízení fungovat jako standardní MIDI kontroler bez potřeby připojení k počítači s Maschine software [^4_3].

USB OTG kabel musí podporovat OTG funkcionalitu - doporučuje se použít originální kabely, protože levnější varianty často nefungují spolehlivě [^4_6][^4_7]. Některé telefony vyžadují povolení OTG v nastavení nebo vývojářských možnostech [^4_8].

### Bluetooth MIDI připojení

Bluetooth MIDI (BLE MIDI) je další možnost připojení, která je podporována na Android od verze 4.3 (API 18) [^4_9]. Maschine zařízení však standardně nepodporují Bluetooth MIDI, takže by bylo nutné použít externí Bluetooth MIDI adaptér nebo implementovat vlastní řešení [^4_10].

