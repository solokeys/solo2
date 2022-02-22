import csv

data = """\
PIO0_0 , 2, FC3_SCK,
PIO0_1 , 2, FC3_CTS_SDA_SSEL0,
PIO0_2 , 1, FC3_TXD_SCL_MISO_WS,
PIO0_3 , 1, FC3_RXD_SDA_MOSI_DATA,
PIO0_4 , 2, FC4_SCK,
PIO0_5 , 2, FC4_RXD_SDA_MOSI_DATA,
PIO0_5 , 8, FC3_RTS_SCL_SSEL1,
PIO0_6 , 1, FC3_SCK,
PIO0_7 , 1, FC3_RTS_SCL_SSEL1,
PIO0_7 , 3, FC5_SCK,
PIO0_7 , 4, FC1_SCK,
PIO0_8 , 1, FC3_SSEL3,
PIO0_8 , 3, FC5_RXD_SDA_MOSI_DATA,
PIO0_9 , 1, FC3_SSEL2,
PIO0_9 , 3, FC5_TXD_SCL_MISO_WS,
PIO0_10, 1, FC6_SCK,
PIO0_10, 4, FC1_TXD_SCL_MISO_WS,
PIO0_11, 1, FC6_RXD_SDA_MOSI_DATA,
PIO0_12, 1, FC3_TXD_SCL_MISO_WS,
PIO0_12, 7, FC6_TXD_SCL_MISO_WS,
PIO0_13, 1, FC1_CTS_SDA_SSEL0,
PIO0_13, 5, FC1_RXD_SDA_MOSI_DATA,
PIO0_14, 1, FC1_RTS_SCL_SSEL1,
PIO0_14, 6, FC1_TXD_SCL_MISO_WS,
PIO0_15, 1, FC6_CTS_SDA_SSEL0,
PIO0_16, 1, FC4_TXD_SCL_MISO_WS,
PIO0_17, 1, FC4_SSEL2,
PIO0_18, 1, FC4_CTS_SDA_SSEL0,
PIO0_19, 1, FC4_RTS_SCL_SSEL1,
PIO0_19, 7, FC7_TXD_SCL_MISO_WS,
PIO0_20, 1, FC3_CTS_SDA_SSEL0,
PIO0_20, 7, FC7_RXD_SDA_MOSI_DATA,
PIO0_20, 8, HS_SPI_SSEL0,
PIO0_21, 1, FC3_RTS_SCL_SSEL1,
PIO0_21, 7, FC7_SCK,
PIO0_22, 1, FC6_TXD_SCL_MISO_WS,
PIO0_23, 5, FC0_CTS_SDA_SSEL0,
PIO0_24, 1, FC0_RXD_SDA_MOSI_DATA,
PIO0_25, 1, FC0_TXD_SCL_MISO_WS,
PIO0_26, 1, FC2_RXD_SDA_MOSI_DATA,
PIO0_26, 8, FC0_SCK,
PIO0_26, 9, HS_SPI_MOSI,
PIO0_27, 1, FC2_TXD_SCL_MISO_WS,
PIO0_27, 7, FC7_RXD_SDA_MOSI_DATA,
PIO0_28, 1, FC0_SCK,
PIO0_29, 1, FC0_RXD_SDA_MOSI_DATA,
PIO0_30, 1, FC0_TXD_SCL_MISO_WS,
PIO0_31, 1, FC0_CTS_SDA_SSEL0,
PIO1_0 , 1, FC0_RTS_SCL_SSEL1,
PIO1_1 , 1, FC3_RXD_SDA_MOSI_DATA,
PIO1_1,  5, HS_SPI_SSEL1,
PIO1_2,  6, HS_SPI_SCK,
PIO1_3,  6, HS_SPI_MISO,
PIO1_4 , 1, FC0_SCK,
PIO1_5 , 1, FC0_RXD_SDA_MOSI_DATA,
PIO1_6 , 1, FC0_TXD_SCL_MISO_WS,
PIO1_7 , 1, FC0_RTS_SCL_SSEL1,
PIO1_8 , 1, FC0_CTS_SDA_SSEL0,
PIO1_8 , 5, FC4_SSEL2,
PIO1_9 , 2, FC1_SCK,
PIO1_9 , 5, FC4_CTS_SDA_SSEL0,
PIO1_10, 2, FC1_RXD_SDA_MOSI_DATA,
PIO1_11, 2, FC1_TXD_SCL_MISO_WS,
PIO1_12, 2, FC6_SCK,
PIO1_12, 5, HS_SPI_SSEL2,
PIO1_13, 2, FC6_RXD_SDA_MOSI_DATA,
PIO1_14, 4, FC5_CTS_SDA_SSEL0,
PIO1_15, 4, FC5_RTS_SCL_SSEL1,
PIO1_15, 5, FC4_RTS_SCL_SSEL1,
PIO1_16, 2, FC6_TXD_SCL_MISO_WS,
PIO1_17, 3, FC6_RTS_SCL_SSEL1,
PIO1_19, 5, FC4_SCK,
PIO1_20, 1, FC7_RTS_SCL_SSEL1,
PIO1_20, 5, FC4_TXD_SCL_MISO_WS,
PIO1_21, 1, FC7_CTS_SDA_SSEL0,
PIO1_21, 5, FC4_RXD_SDA_MOSI_DATA,
PIO1_22, 5, FC4_SSEL3,
PIO1_23, 1, FC2_SCK,
PIO1_23, 5, FC3_SSEL2,
PIO1_24, 1, FC2_RXD_SDA_MOSI_DATA,
PIO1_24, 5, FC3_SSEL3,
PIO1_25, 1, FC2_TXD_SCL_MISO_WS,
PIO1_26, 1, FC2_CTS_SDA_SSEL0,
PIO1_26, 5, HS_SPI_SSEL3,
PIO1_27, 1, FC2_RTS_SCL_SSEL1,
PIO1_28, 1, FC7_SCK,
PIO1_29, 1, FC7_RXD_SDA_MOSI_DATA,
PIO1_30, 1, FC7_TXD_SCL_MISO_WS,"""


def some_lower(name):
    return name[:1].upper() + name[1:].lower()


def first_upper(name):
    return name[:1].upper() + name[1:].lower()


data = [list(map(str.strip, row))[:-1] for row in csv.reader(data.split("\n"))]

# functions = list(sorted(set([row[2] for row in data])))
# for FUNCTION in functions:
#     print(f"""\
# pub struct ${function};
# impl Function for ${FUNCTION} {{}}""")


# Goal: entries like this:
# (Pio0_1, pio0_1): {
#     (2, FC3_CTS_SDA_SSEL0): [
#         (into_usart3_cts_pin, Usart3, UsartCtsPin),
#         (into_i2c3_sda_pin, I2c3, I2cSdaPin),
#         (into_spi_cs_pin, Spi3, SpiCsPin),
#     ]
# }

implementations = []

for PIN, alt, FUNCTION in data:
    pin = PIN.lower()
    Pin = first_upper(pin)
    print(f"({Pin}, {pin}): {{")
    print(f"    ({alt}, {FUNCTION}): [")

    if FUNCTION.startswith("HS"):
        i = 8
        KINDS = [FUNCTION.rsplit("_", 1)[-1]]
        PERIPHERALS = ["SPI"]
    else:
        assert FUNCTION.startswith("FC")
        i = FUNCTION[2]
        KINDS = FUNCTION.split("_")[1:]
        l = len(KINDS)
        assert l in (1, 3, 4), KINDS
        if l == 1:
            if KINDS[0].startswith("SSEL"):
                PERIPHERALS = ["SPI"]
            else:
                assert KINDS[0] == "SCK", KINDS
                PERIPHERALS = ["USART"]

                PERIPHERALS = ["USART", "SPI"]
                KINDS = ["SCLK", "SCK"]
        else:
            PERIPHERALS = ["USART", "I2C", "SPI"]
            if l == 4:
                PERIPHERALS.append("I2S")

    for (KIND, PERIPHERAL) in zip(KINDS, PERIPHERALS):
        # Corrections
        if KIND in ("RXD", "TXD"):
            KIND = KIND[:2]
        if PERIPHERAL == "I2S" and KIND == "DATA":
            KIND = "SDA"
        if PERIPHERAL == "USART" and KIND == "CLK":
            KIND = "SCLK"
        if KIND.startswith("SSEL"):
            chip = KIND[-1]
            KIND = "CS"
        else:
            chip = None

        peripheral = PERIPHERAL.lower()
        Peripheral = first_upper(peripheral)
        Peripherali = f"{Peripheral}{i}"
        kind = KIND.lower()
        Kind = first_upper(kind)
        PeripheralKindPin = f"{Peripheral}{Kind}Pin"

        print(
            f"        (into_{peripheral}{i}_{kind}_pin, {Peripherali}, {PeripheralKindPin}),"
        )

        implementations.append((PeripheralKindPin, Peripherali, FUNCTION, chip))

    print(f"    ]")
    print(f"}}")

# assert len(implementations) > len(set(tuple(implementations)))
# impl SpiSckPin<Pio1_2, Spi8> for Pin<Pio1_2, Special<HS_SPI_SCK>> {}
# impl SpiMosiPin<Pio0_26, Spi8> for Pin<Pio0_26, Special<HS_SPI_MOSI>> {}
for PeripheralKindPin, Peripherali, FUNCTION, chip in sorted(set(implementations)):
    if chip is None:
        print(
            f"impl<PIO: PinId> fc::{PeripheralKindPin}<PIO, flexcomm::{Peripherali}> for Pin<PIO, Special<function::{FUNCTION}>> {{}}"
        )
    else:
        print(
            f"impl<PIO: PinId> fc::{PeripheralKindPin}<PIO, flexcomm::{Peripherali}> for Pin<PIO, Special<function::{FUNCTION}>> {{"
        )
        print(f"    const CS: ChipSelect = ChipSelect::Chip{chip};")
        print(f"}}")
