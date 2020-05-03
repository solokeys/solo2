from smartcard.System import readers

r = readers()[0]

c = r.createConnection()
c.connect()

# CLA, INS, P1, P2, Lc
SELECT = [0xA0, 0xA4, 0x00, 0x00, 0x02]
DF_TELECOM = [0x7F, 0x10]

resp = c.transmit(SELECT + DF_TELECOM)
