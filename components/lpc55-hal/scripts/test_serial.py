import secrets
import sys

import serial

ser = serial.Serial(
    "/dev/ttyACM1",
    115_200,
    # 4_000_000,
    timeout=0,
    write_timeout=0,
)
print("connected")

w = 0
while ser.read():
    w += 1
    pass
print(f"cleared {w} waiting chars")


def loop(sent_msg, verbose=False):
    length = len(sent_msg)
    at = 0
    recv_msg = bytes()
    while (at < length) or (len(recv_msg) < length):
        sent = ser.write(sent_msg[at:])
        at += sent
        if verbose and sent > 0:
            print(f"...wrote {sent}")

        recv = ser.read(at - len(recv_msg))
        got = len(recv)
        if verbose and got > 0:
            print(f"...read {got}")
        recv_msg += recv

    assert recv_msg == sent_msg
    if verbose:
        print(f"...success!")


try:
    print(f"small cases")
    for i in range(32):
        # print(f"small case {i}")
        loop(bytes(range(i)))  # , verbose=True)

    max_length = 65536
    # max_length = 6144
    repeats = 10
    smallest, largest = max_length, 0

    print(f"large cases")
    for i in range(repeats):
        length = secrets.randbelow(max_length + 1)
        msg = bytes([secrets.randbelow(256) for _ in range(length)])
        # print(f"iteration {i}: {length}B")
        if length < smallest:
            smallest = length
        if length > largest:
            largest = length
        loop(msg)

    print(f"Successfully looped {repeats} random msgs of length <= {max_length}")
    print(f"smallest: {smallest}, largest: {largest}")

except AssertionError as e:
    print(f"Error: {e}")
    sys.exit(1)
