from sys import argv

assert argv[1] in {"on", "off"}

from IPython import embed

from smartcard.System import readers
from smartcard.CardConnection import CardConnection
from smartcard.scard import SCARD_SHARE_DIRECT

def b(hex_string):
    return bytes.fromhex(hex_string)

# connect to reader
rs = readers()
r = next((r for r in rs if '[ACR1252 Dual Reader PICC]' in r.name))
c = r.createConnection()
# allow connecting without card
c.connect(protocol=CardConnection.RAW_protocol, mode=SCARD_SHARE_DIRECT)

# assemble and send command to toggle beep
# https://www.acs.com.hk/download-manual/6402/API-ACR1252U-1.16.pdf
#
# We use the "ACR122U compatible commands" (well not really)
#
# p2 turns on/off
#
beep_on, beep_off = b("FF"), b("00")
d = {"on": beep_on, "off": beep_off}
p2 = d[argv[1]]

cla, ins, p1, le = b("FF"), b("00"), b("52"), b("00")
apdu = list(cla + ins + p1 + p2 + b("00"))
print(apdu)
c.transmit(apdu)

embed()
