from smartcard.System import readers

r = readers()[0]

c = r.createConnection()
c.connect()


def hexy(l):
    # return ':'.join([f'{x:x}' for x in l])
    return ' '.join([f'{x:02X}' for x in l])


print(f"ATR = '{hexy(c.getATR())}'")

# CLA, INS, P1, P2, Lc
NIST_RID = [0xA0, 0x00, 0x00, 0x03, 0x08]
NIST_PIX_PIV_APP = [0x00, 0x00, 0x10, 0x00]
NIST_PIX_PIV_VERSION = [0x01, 0x00]
PIV = NIST_RID + NIST_PIX_PIV_APP + NIST_PIX_PIV_VERSION

SELECT = [0x00, 0xA4, 0x04, 0x00, len(PIV)]

resp = c.transmit(SELECT + PIV + [0])


# call e.g. with 0x7E for discovery object,
# 0x7F61 for biometric information template interindustry tag,
# or 0x5FC107 for card capability container, etc.
# except these first two, all tags have length 3,
# and even are [0x5F, 0xC1, ?].
def get_data(object_tag):
    if object_tag == 0x7E:
        tag = [0x7E]
    elif object_tag == 0x7F61:
        tag = list(object_tag.to_bytes(2, byteorder='big'))
    else:
        tag = list(object_tag.to_bytes(3, byteorder='big'))

    GET_DATA = [0x00, 0xCB, 0x3F, 0xFF]
    return c.transmit(GET_DATA + [len(tag) + 2] + [0x5C, 1] + tag + [0])
