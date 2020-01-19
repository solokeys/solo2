import serial
import sys
import time

# while True:
if True:
    try:
        ser = serial.Serial(
            "/dev/ttyACM1",
            115_200,
            # 4_000_000,
            timeout=0,
            write_timeout=0,
        )

        print("connected")

        while True:
            recv = ser.read(64)
            if recv:
                # print(recv, sep="", end="")
                print(recv.decode())
    except serial.SerialException as e:
        print("\n\n==== SERIAL EXCEPTION ====")
        print(e)
        # ser.close()
        time.sleep(0.1)
    except KeyboardInterrupt:
        sys.exit(0)
