import serial
import sys
import time

port = sys.argv[1] if len(sys.argv) > 1 else 1

while True:
    try:
        ser = serial.Serial(
            f"/dev/ttyACM{port}",
            115_200,
            # 4_000_000,
            timeout=0,
            write_timeout=0,
        )

        print("connected")

        while True:
            recv = ser.read(64)
            if recv:
                print(recv.decode(), sep="", end="")
                # print(recv.decode())
            else:
                time.sleep(0.1)

    except serial.SerialException as e:
        print("\n\n==== SERIAL EXCEPTION ====")
        print(e)
        # ser.close()
        time.sleep(0.1)

    except KeyboardInterrupt as e:
        sys.exit(0)
