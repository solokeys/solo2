from smartcard.System import readers

r = readers()[0]

c = r.createConnection()
c.connect()

# CLA, INS, P1, P2, Lc
NIST_RID = [0xA0, 0x00, 0x00, 0x03, 0x08]
NIST_PIX_PIV_APP = [0x00, 0x00, 0x10, 0x00]
NIST_PIX_PIV_VERSION = [0x01, 0x00]
PIV = NIST_RID + NIST_PIX_PIV_APP + NIST_PIX_PIV_VERSION

SELECT = [0xA0, 0xA4, 0x00, 0x00, len(PIV)]

resp = c.transmit(SELECT + PIV)
