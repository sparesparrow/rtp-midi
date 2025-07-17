# **Připojení Maschine k telefonu: Komplexní průvodce integrací s ESP32 a LED systémem**

Tento dokument poskytuje vyčerpávající technickou analýzu a návrh systému pro připojení kontroleru Native Instruments Maschine k telefonu se systémem Android. Telefon bude sloužit jako centrální hub, který v reálném čase zpracovává MIDI data a směruje je jednak do Digital Audio Workstation (DAW) přes Wi-Fi pomocí protokolu RTP-MIDI, a jednak do mikrokontroleru ESP32 pro řízení vizualizace na programovatelném LED pásku. Cílem je vytvořit robustní, přenosný a nízko-latenční systém s vysokým kreativním potenciálem.

## **Sekce 1: Architektura systému a analýza protokolů**

Základem úspěšného a výkonného systému je pečlivě navržená architektura, která zohledňuje specifické požadavky každé komponenty. V této sekci definujeme celkovou strukturu, datové toky a především zdůvodníme výběr klíčových komunikačních protokolů, které tvoří páteř celého řešení.

### **1.1. Vysokoúrovňový diagram systému a datový tok**

Navrhovaný systém se skládá ze čtyř hlavních komponent, jejichž interakce a datové toky jsou znázorněny v následujícím popisu.  
**Diagram datového toku:** Maschine Controller \-\> USB OTG \-\> Android Hub \-\> (Cesta A: Wi-Fi RTP-MIDI \-\> DAW) & (Cesta B: Wi-Fi OSC \-\> ESP32 \-\> LED pásek)  
**Role jednotlivých komponent:**

1. **Maschine Controller (MK2/MK3):** Zdroj MIDI signálu. Zařízení musí být přepnuto do MIDI režimu (podržením **Shift \+ Channel (MIDI)**) , aby fungovalo jako standardní MIDI kontroler a posílalo surová MIDI data bez závislosti na proprietárním softwaru Native Instruments .  
2. **Android Hub (Telefon):** Jádro celého systému. Telefon s custom aplikací připojený k Maschine přes USB OTG . Jeho úkolem je přijímat MIDI data, zpracovávat je a fungovat jako inteligentní router a překladač.  
3. **DAW (Ableton Live, Logic Pro):** Cílová stanice pro hudební produkci. Přijímá MIDI data bezdrátově z Android Hubu, což umožňuje ovládání virtuálních nástrojů a dalších funkcí DAW .  
4. **ESP32 Visualizer:** Koncový bod pro vizualizaci. Mikrokontroler ESP32-WROOM přijímá řídicí příkazy z Android Hubu a v reálném čase generuje efekty na připojeném LED pásku (23 diod WS2812B) .

### **1.2. Kritický výběr protokolu: Hloubkové srovnání**

Volba síťových protokolů je nejdůležitějším architektonickým rozhodnutím, které přímo ovlivní latenci, spolehlivost a složitost implementace. Pro dvě odlišné cesty (do DAW a do ESP32) je nutné zvážit různé protokoly.

* **RTP-MIDI (RFC 6295):** Tento protokol je ideální volbou pro komunikaci mezi Android Hubem a DAW. Jako standard IETF (Internet Engineering Task Force) je navržen specificky pro přenos MIDI 1.0 zpráv v reálném čase přes sítě s možnou ztrátou paketů, jako je Wi-Fi. Jeho klíčovou výhodou je mechanismus "recovery journal", který umožňuje rekonstruovat ztracené pakety bez nutnosti jejich opětovného zaslání, čímž se předchází nárůstu latence a vzniku "zaseknutých not". Je to nativní protokol v systémech Apple (macOS, iOS), což zaručuje bezproblémovou integraci s Logic Pro , a je plně podporován na Windows prostřednictvím ovladačů jako rtpMIDI od Tobiase Erichsena, což zajišťuje kompatibilitu s Ableton Live. Protokol standardně využívá UDP porty 5004 a 5005 pro řízení a data.  
* **Open Sound Control (OSC):** Pro komunikaci mezi Android Hubem a ESP32 je OSC výrazně lepší volbou než samotné MIDI. OSC je lehký, flexibilní a moderní protokol navržený pro komunikaci mezi multimediálními zařízeními. Na rozdíl od MIDI, které má pevně danou strukturu zpráv, OSC používá hierarchický adresní prostor (např. /led/segment/1/color) a podporuje datové typy s vysokým rozlišením, jako jsou 32bitové floaty. To je mnohem efektivnější pro posílání již předzpracovaných vizualizačních příkazů (např. konkrétní RGB barva, rychlost efektu) než posílání surových MIDI dat a jejich následné parsování na hardwarově omezeném ESP32. Existuje řada robustních knihoven pro Arduino (MicroOsc, ArduinoOSC) i pro Javu/Kotlin (JavaOSC) , což zjednodušuje implementaci.  
* **MQTT a WebSockets:** Tyto protokoly jsou pro primární datové cesty v tomto projektu nevhodné. MQTT, ačkoliv je standardem v IoT, je protokol založený na brokeru (publish/subscribe), což z principu vnáší dodatečnou latenci, která je pro hudební aplikace v reálném čase nepřijatelná. WebSockets nabízejí nízko-latenční, plně duplexní spojení, ale jsou postaveny nad TCP, což znamená vyšší overhead (režii) a mechanismy pro zajištění doručení, které nejsou pro streamování velkého množství malých, časově kritických zpráv ideální ve srovnání s čistým UDP. Mohly by však nalézt uplatnění pro sekundární konfigurační kanál.  
* **WLED JSON/HTTP API:** Toto je specifické API pro hotový firmware WLED. Jedná se o protokol typu požadavek-odpověď (request-response) běžící nad TCP/HTTP. Každý příkaz by tak byl zatížen režií HTTP hlaviček a TCP handshake, což by vedlo k vysoké a nepředvídatelné latenci, naprosto nevhodné pro plynulou vizualizaci s vysokou snímkovou frekvencí řízenou MIDI událostmi.

### **1.3. Doporučená "dvouprotokolová" architektura**

Na základě výše uvedené analýzy je zřejmé, že přístup "jeden protokol pro všechno" by byl neefektivní. Optimálním řešením je **dvouprotokolová architektura**, která využívá silných stránek každého protokolu pro specifický účel. Tento přístup je klíčový, neboť cílové systémy (DAW a ESP32) mají diametrálně odlišné požadavky. DAW očekává standardní MIDI stream , zatímco ESP32, jakožto zařízení s omezenými zdroji , těží z jednoduchých a předzpracovaných příkazů.  
Výkonný procesor v telefonu může snadno převzít roli překladače. Tím se složitost parsování MIDI (zpracování polyfonie, časování, mapování CC) přesouvá z ESP32 do Android Hubu, což je klasický vzor distribuovaných systémů, kde se výpočetní zátěž umisťuje tam, kde je to nejefektivnější.

1. **Primární cesta v reálném čase (DAW):** Android \-\> RTP-MIDI (UDP) \-\> DAW. Tato cesta prioritizuje standardní kompatibilitu a minimální zásahy do dat pro co nejnižší latenci směrem k hudebnímu softwaru.  
2. **Sekundární cesta v reálném čase (Visualizer):** Android \-\> OSC (UDP) \-\> ESP32. Tato cesta prioritizuje efektivitu, jednoduchost a nízkou režii pro vestavěné zařízení. Aplikace v Androidu překládá MIDI události na stručné OSC zprávy (např. MIDI Note On (kanál 1, nota 60, velocity 127\) se přeloží na OSC zprávu /led/10/on 255 0 0 250).  
3. **Terciární konfigurační cesta (volitelná):** Pro ne-časově kritické operace, jako je nahrávání nových barevných palet nebo definic efektů z Androidu do ESP32, by mohl být použit protokol založený na TCP, například HTTP nebo WebSockets.

| Protokol | Transport | Profil Latence | Režie (Overhead) | Primární Užití v Projektu | Klíčové Knihovny |
| :---- | :---- | :---- | :---- | :---- | :---- |
| **RTP-MIDI** | UDP | Velmi nízká | Nízká | Android \-\> DAW | libRtpMidi (C++), AppleMIDI (Arduino) |
| **OSC** | UDP | Velmi nízká | Velmi nízká | Android \-\> ESP32 | JavaOSC (Java/Kotlin), ArduinoOSC |
| **MQTT** | TCP | Střední až vysoká | Střední | Nevhodné | Paho, PubSubClient |
| **WebSockets** | TCP | Nízká | Střední | Konfigurace (volitelné) | Různé (Jetty, etc.) |
| **WLED API** | HTTP/TCP | Vysoká | Vysoká | Nevhodné | \- |

### **1.4. Objevování služeb: Implementace mDNS/Bonjour**

Aby se předešlo nutnosti manuálně konfigurovat IP adresy, je nezbytné implementovat mechanismus pro automatické objevování služeb v lokální síti. K tomuto účelu slouží protokol mDNS (Multicast DNS), známý také pod obchodním názvem Bonjour od společnosti Apple.

* **Na Androidu:** Využijeme nativní NsdManager (Network Service Discovery) API. Aplikace v Kotlinu bude registrovat svou vlastní RTP-MIDI službu (např. \_apple-midi.\_udp) a zároveň aktivně vyhledávat OSC službu poskytovanou ESP32 (např. \_osc.\_udp).  
* **Na ESP32:** Použijeme knihovnu ESPmDNS, která je standardní součástí Arduino Core pro ESP32. Firmware na ESP32 zaregistruje svou OSC službu, což umožní Android aplikaci nalézt zařízení pod přívětivým jménem, například esp32-visualizer.local.  
* **Na PC/Mac:** DAW jako Logic Pro automaticky využívají Bonjour k nalezení RTP-MIDI služeb v síti. Pro Ableton Live na Windows tuto funkci zajišťuje ovladač rtpMIDI.

## **Sekce 2: Android MIDI Hub: Průvodce nativní implementací**

Tato sekce se podrobně věnuje návrhu aplikace pro Android, přičemž klade důraz na "hybridní nativní" model, který je nezbytný pro dosažení požadovaného výkonu a nízké latence.

### **2.1. Volba architektury: "Hybridní nativní" model (Kotlin \+ NDK)**

Pro aplikace s tvrdými požadavky na zpracování v reálném čase, jako je profesionální audio, jsou multiplatformní frameworky (např. React Native, Flutter) nevhodné. Důvodem jsou výkonnostní limity, dodatečná vrstva abstrakce a zpožděný přístup k nativním API platformy. Klíčovým požadavkem je přístup k nízko-latenční audio/MIDI cestě Androidu, což vyžaduje použití NDK (Native Development Kit) a programování v C/C++.  
Proto je jednoznačně doporučena **hybridní nativní architektura**:

* **Vrstva Kotlin (UI a řízení):** Uživatelské rozhraní (UI), správa nastavení, objevování síťových služeb (NsdManager) a celkový životní cyklus aplikace budou implementovány v jazyce Kotlin. Pro UI je vhodné použít moderní framework Jetpack Compose a pro čistší kód KTX rozšíření.  
* **Vrstva C++ (Jádro v reálném čase):** Výkonnostně kritické jádro aplikace bude napsáno v C++. Toto jádro bude zodpovědné za:  
  1. Komunikaci s USB MIDI zařízením pomocí AMidi NDK API pro nejnižší možnou latenci.  
  2. Běh hlavní zpracovávací smyčky na vlákně s vysokou prioritou, aby se předešlo zpožděním (jitter) způsobeným Garbage Collectorem (GC) virtuálního stroje ART/Dalvik.  
  3. Parsování MIDI zpráv.  
  4. Sestavování a odesílání RTP-MIDI paketů přes UDP socket.  
  5. Překlad MIDI na OSC a odesílání OSC paketů přes druhý UDP socket.  
* **JNI Bridge (Java Native Interface):** Jasně definované JNI rozhraní propojí vrstvu Kotlin a C++. Kotlin bude toto rozhraní používat ke spouštění/zastavování jádra, předávání konfigurace (IP adresy, porty, nastavení vizualizace) a přijímání stavových informací.

### **2.2. Jádro aplikace: Služba na popředí a oprávnění**

Aby MIDI směrování spolehlivě fungovalo i v případě, že je aplikace na pozadí, je povinné použít **Službu na popředí (Foreground Service)**.  
Na Androidu 14 a novějších verzích to vyžaduje deklaraci specifického typu služby. Nejvhodnějším typem je connectedDevice. Je nutné provést následující kroky:

* **Deklarace v AndroidManifest.xml:**  
  * \<uses-permission android:name="android.permission.FOREGROUND\_SERVICE" /\>  
  * \<uses-permission android:name="android.permission.FOREGROUND\_SERVICE\_CONNECTED\_DEVICE" /\>  
  * V elementu \<service\> uvést android:foregroundServiceType="connectedDevice".  
* **Runtime logika:** V kódu je nutné volat metodu startForeground() se správným typem služby a zobrazit uživateli trvalou notifikaci, což je systémový požadavek.  
* **Další oprávnění:** Bude nutné deklarovat a případně za běhu vyžádat oprávnění pro přístup k internetu (INTERNET), stavu Wi-Fi (ACCESS\_NETWORK\_STATE, CHANGE\_WIFI\_MULTICAST\_STATE pro mDNS) a přístup k USB (UsbManager.requestPermission()).

### **2.3. Integrace MIDI a síťových knihoven**

* **USB MIDI:** Primárním rozhraním bude nativní AMidi API z NDK pro nejnižší latenci. Pro objevování zařízení a vyžádání oprávnění před předáním handle do NDK vrstvy se využije Java android.media.midi balíček (MidiManager).  
* **RTP-MIDI:** Ačkoliv neexistuje hotová kompletní RTP-MIDI knihovna pro Kotlin/Android, je možné implementovat jádro protokolu RFC 6295 v C++ části. Knihovna ktmidi je vynikající pro parsování a konstrukci MIDI zpráv, ale neřeší samotnou RTP vrstvu a správu sezení. Jako vodítko pro implementaci struktury paketů a logiky správy sezení mohou posloužit Java implementace jako nmj nebo C++ knihovny jako libRtpMidi.  
* **OSC:** Pro OSC cestu se doporučuje použít robustní Java/Kotlin knihovnu jako JavaOSC. Samotný překlad z MIDI na OSC proběhne v C++ jádře, ale konstrukci paketů lze delegovat na C++ OSC knihovnu.

## **Sekce 3: Jádro ESP32: Návrh firmwaru a hardwarová abstrakce**

Tato sekce se zaměřuje na firmware pro ESP32 s důrazem na výkon v reálném čase, efektivní ovládání LED a splnění požadavku na přenositelnost kódu mezi různými deskami ESP32.

### **3.1. Volba frameworku: Arduino Core vs. ESP-IDF**

* **Arduino Core:** Nabízí rychlý vývoj, obrovské množství dostupných knihoven (jako FastLED, AppleMIDI, PubSubClient) a jednodušší křivku učení. Vzhledem k dostupnosti kvalitních knihoven pro všechny potřebné funkce je pro tento projekt Arduino Core nejefektivnější a doporučenou volbou.  
* **ESP-IDF (Espressif IoT Development Framework):** Jedná se o nativní framework od výrobce čipu, Espressif. Nabízí absolutně nejvyšší výkon a nejpřímější přístup k hardwarovým funkcím (např. přiřazení úloh konkrétním jádrům). Ačkoliv je výkonnější, má strmější křivku učení a integrace knihoven by vyžadovala více manuální práce. Arduino Core je postaven nad ESP-IDF, takže výkonnostně kritické funkce jako FreeRTOS úlohy jsou stále k dispozici.

### **3.2. Návrh "konfiguračně řízené" hardwarové abstrakce**

Pro splnění požadavku na zamezení duplikace kódu je navržena **"konfiguračně řízená" architektura** namísto komplexní hardwarové abstrakční vrstvy (HAL).  
Jádro logiky (zpracování sítě, parsování OSC, generování LED efektů) bude napsáno hardwarově agnostickým způsobem. Všechny hardwarově specifické definice (čísla pinů, typ LED pásku atd.) budou izolovány v samostatných hlavičkových souborech (např. board\_config\_devkit\_v1.h, board\_config\_huzzah32.h). Hlavní program (.ino soubor) pak pomocí direktivy preprocesoru (\#include) zahrne příslušný konfigurační soubor.  
**Příklad board\_config\_devkit\_v1.h:**  
`#pragma once`

`#define LED_PIN       16`  
`#define NUM_LEDS      23`  
`#define LED_TYPE      WS2812B`  
`#define COLOR_ORDER   GRB`  
`#define BRIGHTNESS    150`  
`#define VOLTS         5`  
`#define MAX_AMPS      1500 // 23 LEDs * 60mA/LED = 1380mA`

`#define BUILTIN_LED   2`

Tento přístup umožňuje podporu nové desky pouhým vytvořením nového konfiguračního souboru, což je jednoduché, efektivní a přímo řeší zadaný požadavek.

| GPIO Číslo | Primární Funkce | Strapping Pin? | ADC/DAC Schopnost | Bezpečné pro Běžné Použití? | Poznámky/Varování |
| :---- | :---- | :---- | :---- | :---- | :---- |
| GPIO0 | Boot Mode | Ano | ADC1\_CH1 | Ne | Při startu musí být HIGH. Nepoužívat pro výstupy. |
| GPIO1 | TXD0 | Ne | \- | Ano | Výchozí sériový port. Lze použít, pokud se nepoužívá pro ladění. |
| GPIO2 | \- | Ano | ADC2\_CH2 | Ano (jako výstup) | Při startu musí být LOW. Spojen s vestavěnou LED na mnoha deskách. |
| GPIO3 | RXD0 | Ne | \- | Ano | Výchozí sériový port. Lze použít, pokud se nepoužívá pro ladění. |
| GPIO4 | \- | Ne | ADC2\_CH0 | Ano | \- |
| GPIO5 | VSPI\_CS0 | Ano | ADC2\_CH3 | Ano | Při startu musí být HIGH. |
| GPIO6-11 | SPI Flash | Ne | \- | Ne | Interně použito pro komunikaci s flash pamětí. Nepoužívat. |
| GPIO12 | \- | Ano | ADC2\_CH5 | Ano | Při startu musí být LOW pro 3.3V flash. |
| GPIO13 | \- | Ne | ADC2\_CH4 | Ano | \- |
| GPIO14 | \- | Ne | ADC2\_CH6 | Ano | \- |
| GPIO15 | \- | Ano | ADC2\_CH7 | Ano | Při startu musí být HIGH. |
| **GPIO16** | **\-** | **Ne** | **\-** | **Ano (Doporučeno)** | **Dobrá volba pro data LED pásku.** |
| GPIO17-33 | \- | Ne | Různé | Ano | Většina těchto pinů je bezpečná pro obecné použití. |
| GPIO34-39 | Vstupní | Ne | ADC1 | Pouze vstup | Tyto piny nemají interní pull-up/pull-down a nelze je použít jako výstupy. |

### **3.3. Ovládání LED s knihovnou FastLED**

Zatímco WLED je vynikající hotová *aplikace*, pro tento projekt, který vyžaduje vlastní logiku, není vhodnou *knihovnou*. Průmyslovým standardem pro programové ovládání adresovatelných LED je knihovna **FastLED**. Je vysoce optimalizovaná, podporuje širokou škálu čipsetů (WS2812B, APA102 atd.) a poskytuje výkonné nástroje pro práci s barvami a animacemi.

* **Základní funkce:**  
  * FastLED.addLeds\<LED\_TYPE, LED\_PIN, COLOR\_ORDER\>(leds, NUM\_LEDS); pro inicializaci pásku s použitím parametrů z konfiguračního souboru.  
  * Pole CRGB leds; slouží jako framebuffer pro manipulaci s barvami pixelů.  
  * FastLED.show(); zapíše data z framebufferu na pásek.  
* **Správa více pásků/segmentů:** CRGBSet umožňuje definovat logické segmenty v rámci jednoho fyzického pásku, nebo lze definovat více polí CRGB pro ovládání fyzicky oddělených pásků, což umožňuje složitější vizualizace.

### **3.4. Architektura firmwaru: Správa úloh v reálném čase**

ESP32 běží na operačním systému FreeRTOS. Jednoduchý přístup založený na funkci loop() není pro spolehlivou síťovou komunikaci a animace v reálném čase dostatečný. Je navržena **dvouúlohová architektura** s využitím FreeRTOS API, které je dostupné v Arduino Core:

* **Úloha 1 (Síťová úloha):** Běží na jádře 0\. Jedinou zodpovědností této úlohy je naslouchat příchozím UDP paketům (s OSC zprávami), parsovat je a vkládat výsledné příkazy do fronty bezpečné pro přístup z více vláken (xQueueCreate). Tím se izoluje latence sítě od animační smyčky.  
* **Úloha 2 (Animační úloha):** Běží na jádře 1\. Tato úloha běží v těsné smyčce. Kontroluje frontu na nové příkazy ze síťové úlohy a aktualizuje CRGB framebuffer. Je také zodpovědná za časově závislé animace (jako prolínání nebo pulzování) a volání FastLED.show() s pevnou frekvencí (např. 60+ FPS).

Tato dvoujádrová, úlohová architektura zabraňuje tomu, aby síťové problémy způsobovaly vizuální trhání, a zajišťuje nejvyšší možnou snímkovou frekvenci animací.

## **Sekce 4: Síťový most: Propojení Androidu a ESP32**

Tato sekce se zaměřuje na praktickou implementaci komunikačních protokolů zvolených v Sekci 1 a detailně popisuje strukturu kódu na straně Androidu i ESP32.

### **4.1. Implementace protokolu OSC**

* **Návrh OSC zpráv:** Je nutné definovat jasný a efektivní adresní prostor OSC pro ovládání LED.  
  * /noteOn \<int note\> \<int velocity\>  
  * /noteOff \<int note\>  
  * /cc \<int controller\> \<int value\>  
  * /pitchBend \<float bendValue\> (rozsah \-1.0 až 1.0)  
  * /config/setPalette \<string name\>  
  * /config/setEffect \<string name\>  
* **Na straně Androidu (Kotlin/Java):** S použitím knihovny jako JavaOSC se ukáže, jak sestavit a odeslat tyto OSC pakety z vrstvy Kotlin (nebo z C++ jádra) na objevenou IP adresu a port ESP32.  
* **Na straně ESP32 (C++):** S použitím Arduino OSC knihovny (např. ArduinoOSC nebo MicroOsc ) se ukáže, jak nastavit UDP listener a přiřadit příchozí OSC zprávy obslužným funkcím (handlerům).  
  `// Příklad obsluhy OSC na ESP32`  
  `#include <ArduinoOSC.h>`

  `void onNoteOn(const OscMessage& msg) {`  
    `int note = msg.arg<int>(0);`  
    `int velocity = msg.arg<int>(1);`  
    `// Zde se volá vizualizační logika...`  
  `}`

  `void setup() {`  
    `//... nastavení WiFi, atd.`  
    `Osc.subscribe(8000, "/noteOn", onNoteOn); // Naslouchá na portu 8000`  
  `}`

  `void loop() {`  
    `Osc.update(); // Kontroluje nové pakety`  
  `}`

### **4.2. Implementace protokolu RTP-MIDI (Android \-\> DAW)**

* **Správa sezení (Session Management):** Detailně bude popsán proces "handshake" na dvou portech (řídicí a datový), jak vyžaduje specifikace AppleMIDI. Aplikace v Androidu bude fungovat jako "Session Initiator".  
  * **Řídicí port (např. 5004):** Odesílá IN (Invitation), přijímá OK/NO.  
  * **Datový port (např. 5005):** Po úspěchu na řídicím portu odesílá IN, přijímá OK/NO.  
* **Synchronizace:** Bude vysvětlena sekvence synchronizace hodin CK0, CK1, CK2, která se používá k výpočtu latence sítě a synchronizaci časových značek. To je klíčové pro časování s přesností na vzorek v DAW.  
* **Struktura paketu:** Bude rozebrán formát RTP-MIDI paketu podle RFC 6295, včetně toho, jak vkládat MIDI příkazy do RTP payloadu a jak funguje "recovery journal" pro odolnost proti ztrátě paketů.  
* **Volba knihovny:** Pro pochopení protokolu se doporučuje použít knihovnu AppleMIDI pro Arduino/ESP32 jako referenční implementaci. Na straně Androidu bude tato logika muset být implementována v C++ jádře, protože neexistuje hotová Kotlin knihovna pro kompletní správu RTP-MIDI sezení.

### **4.3. Správa spojení a spolehlivost**

* **Heartbeats:** Pro OSC spojení, které je bezstavové (UDP), by měla aplikace v Androidu periodicky posílat "heartbeat" zprávu (např. /ping) na ESP32. Pokud ESP32 po určitou dobu neobdrží heartbeat, může předpokládat, že spojení bylo ztraceno, a přejít do záložního stavu (např. výchozí animace).  
* **Logika znovupřipojení:** Aplikace v Androidu by měla pomocí mDNS neustále monitorovat přítomnost služeb ESP32 a DAW. Pokud služba zmizí a znovu se objeví, aplikace by se měla automaticky pokusit znovu navázat sezení (RTP-MIDI) nebo obnovit odesílání dat (OSC).

## **Sekce 5: Pokročilé techniky vizualizace MIDI na LED**

Tato sekce přechází od infrastruktury k umělecké stránce projektu a popisuje metody pro převod MIDI dat na působivé vizuální efekty. Inspiraci čerpáme z existujících projektů jako music-to-led a MAINFRAME.

### **5.1. Rámec pro mapování MIDI na vizuální efekty**

Ve firmwaru ESP32 bude navržen stavový mapovací rámec. To zahrnuje vytvoření datových struktur pro sledování stavu každé ze 128 možných MIDI not (zapnuto/vypnuto, velocity, časová značka). 23 LED diod bude mapováno na specifický rozsah not na klaviatuře, pravděpodobně na chromatickou stupnici v rozsahu dvou oktáv.

### **5.2. Vizualizace polyfonie a stavu not**

* **Note On:** Když je přijata zpráva /noteOn, rozsvítí se příslušná LED. Velocity noty se přímo mapuje na jas LED. Barva může být pevně daná nebo určena výškou tónu.  
* **Note Off:** Když je přijata zpráva /noteOff, LED se nevypne okamžitě. Estetičtější je postupný fade-out (stmívání). Firmware musí být schopen spravovat více současně probíhajících fade-out animací.  
* **Sustain (CC 64):** Když je stisknut sustain pedál, události noteOff by neměly spouštět fade-out. LED by měly zůstat rozsvícené, dokud není pedál uvolněn. To vyžaduje sledování stavu sustain pedálu.  
* **Polyfonie:** Systém musí zvládat současné hraní více not. Zde je klíčové pole se stavy not. Výsledná barva/jas každé LED může být výsledkem smíchání barev z více aktivních not, které se na ni mapují, což vytváří vizuálně komplexní akordy.

### **5.3. Mapování Velocity, Aftertouch a Pitch Bend**

* **Velocity:** Jak již bylo zmíněno, je to primární ovladač jasu na úrovni jednotlivých not.  
* **Pitch Bend:** Lze vizualizovat jako globální efekt. Například ohnutí tónu nahoru by mohlo způsobit, že se světelná "vlna" pohne zleva doprava po pásku, zatímco ohnutí dolů by ji posunulo zprava doleva. Barva vlny by mohla být ovlivněna mírou ohnutí.  
* **Aftertouch (Channel Pressure):** Ideální pro efekt "pulzování" nebo "žhnutí". Jak se zvyšuje tlak na již drženou klávesu, příslušná LED (nebo celý pásek) může jemně zvyšovat a snižovat svůj jas.

### **5.4. Využití CC zpráv pro dynamické ovládání**

Zprávy Control Change (CC) jsou klíčem k tomu, aby byla vizualizace interaktivní a performativní. Navrhuje se mapování specifických CC čísel na ovládání globálních parametrů vizualizačního enginu, zasílaných přes OSC z aplikace v Androidu (např. /cc 74 127).

* **CC 74 (Filter Cutoff):** Mohl by ovládat odstín (hue) celé barevné palety.  
* **CC 71 (Resonance):** Mohl by ovládat rychlost nebo intenzitu efektu (např. rychlost posunu nebo ostrost pulzu).  
* **Program Change:** Mohl by se použít k přepínání mezi různými předdefinovanými vizualizačními efekty (např. "Piano Scroll" vs. "Spectrum").

## **Sekce 6: End-to-End optimalizace výkonu a minimalizace latence**

Tato sekce se hlouběji zabývá identifikací a minimalizací latence v každé fázi signálového řetězce, což je nezbytné pro transformaci systému z prototypu na nástroj profesionální úrovně.

### **6.1. Holistický pohled na zdroje latence**

Latence se sčítá v celém řetězci: interval pollingu USB, zpoždění plánovače v OS Android, režie volání JNI, doba zpracování v C++, latence sítě (Wi-Fi), síťový jitter, doba zpracování na ESP32 a obnovovací frekvence LED pásku.

### **6.2. Optimalizace Android Hubu**

* **Threading:** Připnutí C++ MIDI jádra na specifické vysoce výkonné jádro CPU (pokud je to možné) a nastavení priority vlákna na THREAD\_PRIORITY\_URGENT\_AUDIO.  
* **Správa bufferů:** Použití optimálních velikostí bufferů pro audio/MIDI, jak je hlásí AudioManager API, aby se aplikace kvalifikovala pro "fast track" (nízko-latenční cestu).  
* **Efektivita JNI:** Minimalizace frekvence a objemu dat JNI volání mezi Kotlinem a C++. Konfigurační data by se měla předávat jednou na začátku, nikoliv v reálném čase.  
* **Optimalizace kódu:** Psaní "GC-friendly" kódu v Kotlinu (v UI vrstvě), aby se minimalizovala frekvence a doba trvání událostí garbage collection.

### **6.3. Optimalizace síťového transportu**

* **Wi-Fi prostředí:** Je klíčové zajistit čisté Wi-Fi prostředí, ideálně s použitím 5GHz sítě s minimálním rušením. ESP32 může být nakonfigurován pro připojení k dedikovanému Access Pointu vytvořenému telefonem pro stabilnější spojení. Je možné nastavit ESP32 do režimu Station (WIFI\_STA) pro připojení k existující síti, nebo do režimu Access Point (WIFI\_AP) pro přímé spojení.  
* **Zpracování UDP paketů:** Pro OSC data je důležité zajistit, že UDP sockety jsou nakonfigurovány pro nízkou latenci.

### **6.4. Optimalizace firmwaru ESP32**

* **Běh z IRAM:** Umístění výkonnostně kritických funkcí (jako je obsluha OSC a jádro renderovací smyčky LED) do IRAM pomocí direktivy IRAM\_ATTR. Tím se zabrání "cache misses" při čtení kódu z flash paměti, což je významný zdroj trhání.  
* **Afinita k jádrům:** Použití xTaskCreatePinnedToCore k explicitnímu připnutí síťové úlohy na jádro 0 a animační úlohy na jádro 1, což zabraňuje migraci úloh a zajišťuje dedikované zdroje.  
* **Výkon FastLED:** Používání neblokujících animačních technik. Za každou cenu se vyhnout použití delay(). Animační smyčka by měla být řízena časovači založenými na millis().  
* **Kompilační příznaky:** Nastavení úrovně optimalizace kompilátoru na \-O2 (Optimize for performance) v Arduino IDE nebo platformio.ini.

## **Sekce 7: Syntéza a fázový implementační plán**

Tato závěrečná sekce poskytuje praktický, krok-za-krokem plán pro sestavení a testování projektu, který zajišťuje logický postup od jednoduchých komponent k plně integrovanému systému.

* **Fáze 1: Základní I/O a samostatné komponenty**  
  1. **Android:** Vytvořit základní aplikaci, která detekuje Maschine přes USB OTG a loguje příchozí MIDI zprávy na obrazovku pomocí android.media.midi API.  
  2. **ESP32:** Napsat samostatný FastLED program, který cykluje různé animace na 23-LED pásku, aby se ověřil hardware a napájení.  
* **Fáze 2: Budování síťového mostu**  
  1. **ESP32:** Implementovat OSC server na ESP32. Vytvořit jednoduchou testovací aplikaci (např. v Pythonu na PC) pro odesílání OSC zpráv a ověření, že ESP32 správně reaguje.  
  2. **Android:** Integrovat OSC klient knihovnu do Android aplikace. Přidat tlačítko pro odeslání testovací OSC zprávy na pevně zadanou IP adresu ESP32.  
  3. **Objevování:** Implementovat mDNS na Android aplikaci i na ESP32 pro automatické objevování, čímž se odstraní potřeba pevně zadané IP adresy.  
* **Fáze 3: Integrace celého řetězce**  
  1. Spojit MIDI vstup z Fáze 1 s OSC výstupem z Fáze 2 v Android aplikaci. Aplikace by nyní měla přijímat MIDI z Maschine a v reálném čase je překládat na OSC příkazy pro ESP32.  
  2. Implementovat logiku RTP-MIDI klienta v Android aplikaci a otestovat konektivitu s DAW na PC/Mac.  
* **Fáze 4: Vylepšení a pokročilé funkce**  
  1. Implementovat pokročilé vizualizační techniky ze Sekce 5 (polyfonie, prolínání, ovládání pomocí CC).  
  2. Vybudovat UI v Androidu pro konfiguraci efektů, palet a síťových nastavení.  
  3. Provést end-to-end optimalizace latence popsané v Sekci 6\.

Tento komplexní návrh poskytuje robustní základ pro vytvoření unikátního a výkonného audio-vizuálního nástroje. Kombinace mobility telefonu, síťových schopností ESP32 a vizuální zpětné vazby LED systému otevírá dveře k novým formám kreativního vyjádření.

#### **Works cited**

1\. RTP-MIDI, https://midi.org/rtp-midi 2\. RTP MIDI: An RTP Payload Format for MIDI \- John Lazzaro, https://john-lazzaro.github.io/rtpmidi/ 3\. RTP-MIDI \- Wikipedia, https://en.wikipedia.org/wiki/RTP-MIDI 4\. FastLED LED animation library for Arduino (formerly FastSPI\_LED), https://fastled.io/ 5\. ESP DevKits \- Espressif Systems, https://www.espressif.com/en/products/devkits 6\. rtpMIDI | Tobias Erichsen, https://www.tobias-erichsen.de/software/rtpmidi.html 7\. atsushieno/ktmidi: Kotlin multiplatform library for MIDI access abstraction and data processing for MIDI 1.0, MIDI 2.0, SMF, SMF2 (MIDI Clip File), and MIDI-CI. \- GitHub, https://github.com/atsushieno/ktmidi 8\. OSC | Derivative \- TouchDesigner, https://derivative.ca/UserGuide/OSC 9\. OSW Manual OpenSound Control (OSC) \- Open Sound World, https://osw.sourceforge.net/html/osc.htm 10\. MicroOsc is a minimal Open Sound Control (OSC) library for Arduino \- GitHub, https://github.com/thomasfredericks/MicroOsc 11\. Esp32 vs Arduino: The Differences \- All3DP, https://all3dp.com/2/esp32-vs-arduino-differences/ 12\. hideakitai/ArduinoOSC: OSC subscriber / publisher for Arduino \- GitHub, https://github.com/hideakitai/ArduinoOSC 13\. Oscleton, an Ableton Live companion app | by Arthur Vimond \- Medium, https://medium.com/@arthurvimond/oscleton-an-ableton-live-companion-app-b9accaafa023 14\. hoijui/JavaOSC: OSC content format/"protocol" library for ... \- GitHub, https://github.com/hoijui/JavaOSC 15\. WebSocket vs. MQTT vs. CoAP: Which is the Best Protocol? \[2023 Update\] \- Nabto, https://www.nabto.com/websocket-vs-mqtt-vs-coap/ 16\. MQTT vs WebSocket \- Which protocol to use when in 2024 \- Ably Realtime, https://ably.com/topic/mqtt-vs-websocket 17\. which protocol to use MQTT or websockets : r/IOT \- Reddit, https://www.reddit.com/r/IOT/comments/1bobhbo/which\_protocol\_to\_use\_mqtt\_or\_websockets/ 18\. WLED \- Powered by the Community \- Espressif Developer Portal, https://developer.espressif.com/blog/2025/01/wled-powered-by-the-community/ 19\. JSON API \- WLED Project, https://kno.wled.ge/interfaces/json-api/ 20\. WLED JSON API \- Misc Forum \- Cycling '74, https://cycling74.com/forums/wled-json-api 21\. Use network service discovery | Connectivity \- Android Developers, https://developer.android.com/develop/connectivity/wifi/use-nsd 22\. Network service discovery with Kotlin | by Richa Shah \- Medium, https://medium.com/@shahricha723/network-service-discovery-with-kotlin-c59362fd843e 23\. Appstractive/dns-sd-kt: Kotlin multiplatform implemention of DNS Service Discovery \- GitHub, https://github.com/Appstractive/dns-sd-kt 24\. mDNS for ESP32 \- RNT Lab, https://rntlab.com/question/mdns-for-esp32/ 25\. How to set up mDNS on an ESP32 \- Last Minute Engineers, https://lastminuteengineers.com/esp32-mdns-tutorial/ 26\. Understanding mDNS on ESP32: Local Network Device Discovery Made Easy, https://ibrahimmansur4.medium.com/understanding-mdns-on-esp32-local-network-device-discovery-made-easy-9aab590f0eea 27\. Native vs Cross-Platform: Development Speed Comparison \- Sidekick Interactive, https://www.sidekickinteractive.com/mobile-app-strategy/native-vs-cross-platform-development-speed-comparison/ 28\. Native vs. Cross-platform App Development: Which Should You Choose? \- NIX United, https://nix-united.com/blog/native-vs-cross-platform-app-development/ 29\. Cross-platform and native app development: How do you choose? | Kotlin Multiplatform, https://www.jetbrains.com/help/kotlin-multiplatform-dev/native-and-cross-platform.html 30\. Android Low Latency Audio |, https://www.januscole.com/about/ 31\. Audio latency | Android NDK, https://developer.android.com/ndk/guides/audio/audio-latency 32\. Android KTX | Kotlin \- Android Developers, https://developer.android.com/kotlin/ktx 33\. MIDI | Android Open Source Project, https://source.android.com/docs/core/audio/midi 34\. Low-latency audio playback on Android \- Stack Overflow, https://stackoverflow.com/questions/14842803/low-latency-audio-playback-on-android 35\. Foreground service types are required | Android Developers, https://developer.android.com/about/versions/14/changes/fgs-types-required 36\. Guide to Foreground Services on Android 14 \- Medium, https://medium.com/@domen.lanisnik/guide-to-foreground-services-on-android-9d0127dc8f9a 37\. android.media.midi | API reference \- Android Developers, https://developer.android.com/reference/android/media/midi/package-summary 38\. libRtpMidi \- Tobias Erichsen, https://www.tobias-erichsen.de/software/librtpmidi.html 39\. Step-by-Step Guide: How to Program ESP32 with Arduino IDE? \- m5stack-store, https://shop.m5stack.com/blogs/news/step-by-step-guide-how-to-program-esp32-with-arduino-ide 40\. ESP32 vs Arduino \- Reddit, https://www.reddit.com/r/esp32/comments/1bhrf2d/esp32\_vs\_arduino/ 41\. FastLED Library \- FastLED, https://fastled.io/docs/ 42\. flashing different groups on one strip \- Adafruit Forums, https://forums.adafruit.com/viewtopic.php?t=131534 43\. Multiple Controller Examples · FastLED/FastLED Wiki · GitHub, https://github.com/fastled/fastled/wiki/multiple-controller-examples 44\. AppleMIDI \- Arduino Documentation, https://docs.arduino.cc/libraries/applemidi/ 45\. AppleMIDI.ino \- Control Surface, https://tttapa.github.io/Control-Surface/Doxygen/dc/d7d/AppleMIDI\_8ino-example.html 46\. lathoub/Arduino-AppleMIDI-Library: Send and receive MIDI messages over Ethernet (rtpMIDI or AppleMIDI) \- GitHub, https://github.com/lathoub/Arduino-AppleMIDI-Library 47\. tfrere/music-to-led: Create real-time audio and midi ... \- GitHub, https://github.com/tfrere/music-to-led 48\. Piano & LED Strip Visualization \- MAINFRAME, https://www.mainframesys.com/post/piano-led-strip-visualization 49\. MIDI Keyboards & Polyphony \- Music & Patches \- VCV Community, https://community.vcvrack.com/t/midi-keyboards-polyphony/12179 50\. The Ultimate Guide to Latency in Music \- Number Analytics, https://www.numberanalytics.com/blog/ultimate-guide-latency-music-education 51\. ESP32 Useful Wi-Fi Library Functions (Arduino IDE) \- Random Nerd Tutorials, https://randomnerdtutorials.com/esp32-useful-wi-fi-functions-arduino/ 52\. How to Set an ESP32 Access Point (AP) for Web Server \- Random Nerd Tutorials, https://randomnerdtutorials.com/esp32-access-point-ap-web-server/ 53\. Creating a Wireless Network With ESP32 AP Mode \- Instructables, https://www.instructables.com/Creating-a-Wireless-Network-With-ESP32-AP-Mode/ 54\. Speed Optimization \- ESP32 \- — ESP-IDF Programming Guide v5.4.2 documentation, https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/performance/speed.html 55\. Speed Optimization \- ESP32-H2 \- — ESP-IDF Programming Guide v5.4.2 documentation, https://docs.espressif.com/projects/esp-idf/en/stable/esp32h2/api-guides/performance/speed.html