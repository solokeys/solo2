/**
 * Copyright (c) 2022, Nitrokey GmbH
 */


#ifndef NK_SDK_CONFIG_H
#define NK_SDK_CONFIG_H
// <<< Use Configuration Wizard in Context Menu >>>\n
#ifdef USE_APP_CONFIG
#include "app_config.h"
#endif

//
// Nitrokey 3 configuration
//
// Only the modified #defines are listed here, below this you will find
// the default #defines provided by the original sdk_config.h
//


// USB VID:PID
//==========================================================
#ifndef APP_USBD_VID
#define APP_USBD_VID 0x20A0
#endif

#ifndef APP_USBD_PID
#define APP_USBD_PID 0x42DD
#endif

// USB manufacturer string
//==========================================================
#ifndef APP_USBD_STRING_ID_MANUFACTURER
#define APP_USBD_STRING_ID_MANUFACTURER 1
#endif

#ifndef APP_USBD_STRINGS_MANUFACTURER
#define APP_USBD_STRINGS_MANUFACTURER APP_USBD_STRING_DESC("Nitrokey")
#endif

// USB description string
//==========================================================
#ifndef APP_USBD_STRING_ID_PRODUCT
#define APP_USBD_STRING_ID_PRODUCT 2
#endif
#ifndef APP_USBD_STRINGS_PRODUCT
#define APP_USBD_STRINGS_PRODUCT APP_USBD_STRING_DESC("Nitrokey 3 Bootloader r2")
#endif

// USB serial configuration
//==========================================================
#ifndef APP_USBD_STRING_ID_SERIAL
#define APP_USBD_STRING_ID_SERIAL 3
#endif

#ifndef APP_USBD_STRING_SERIAL_EXTERN
#define APP_USBD_STRING_SERIAL_EXTERN 1
#endif

#ifndef APP_USBD_STRING_SERIAL
#define APP_USBD_STRING_SERIAL g_extern_serial_number
#endif

// Bootloader security mechanisms
//==========================================================

// nrfutil hardware version to set during DFU upload to not accidently upload the wrong firmware
#ifndef NRF_DFU_HW_VERSION
#define NRF_DFU_HW_VERSION 52
#endif

// Require a valid signature to update the application or SoftDevice.
#ifndef NRF_DFU_REQUIRE_SIGNED_APP_UPDATE
#define NRF_DFU_REQUIRE_SIGNED_APP_UPDATE 1
#endif

// Perform signature check on the app. Requires the signature to be sent in the init packet.
// @TODO: don't understand this, regular update does not work with this being activated
#ifndef NRF_BL_APP_SIGNATURE_CHECK_REQUIRED
#define NRF_BL_APP_SIGNATURE_CHECK_REQUIRED 0
#endif

// Disable access to the chip via the debug port. (APPROTECT & DEBUGCTRL)
#ifndef NRF_BL_DEBUG_PORT_DISABLE
#define NRF_BL_DEBUG_PORT_DISABLE 0
#endif

// Whether to accept application upgrades with the same version as the current application.
#ifndef NRF_DFU_APP_ACCEPT_SAME_VERSION
#define NRF_DFU_APP_ACCEPT_SAME_VERSION 1
#endif

// Check the firmware version and SoftDevice requirements of application (and SoftDevice) updates.
#ifndef NRF_DFU_APP_DOWNGRADE_PREVENTION
#define NRF_DFU_APP_DOWNGRADE_PREVENTION 1
#endif


// How to enter the Bootloader
//==========================================================

// Enter DFU mode on button press.
#ifndef NRF_BL_DFU_ENTER_METHOD_BUTTON
#define NRF_BL_DFU_ENTER_METHOD_BUTTON 0
#endif

// Enter DFU mode when bit 0 is set in the NRF_POWER_GPREGRET register.
// Preferred way to enter the bootloader
#ifndef NRF_BL_DFU_ENTER_METHOD_GPREGRET
#define NRF_BL_DFU_ENTER_METHOD_GPREGRET 1
#endif

// Various Bootloader properties
//==========================================================

// <o> NRF_DFU_APP_DATA_AREA_SIZE - The size (in bytes) of the flash area reserved for application data.
// <i> This area is found at the end of the application area, next to the start of
// <i> the bootloader. This area will not be erased by the bootloader during a
// <i> firmware upgrade. The size must be a multiple of the flash page size.
#ifndef NRF_DFU_APP_DATA_AREA_SIZE
#define NRF_DFU_APP_DATA_AREA_SIZE   CODE_PAGE_SIZE*20
#endif

// Timeout in ms before automatically starting a valid application due to inactivity.  <0-60000000>
// If 0, no inactivity timer will be used. Values 1-99 are invalid.
#ifndef NRF_BL_DFU_INACTIVITY_TIMEOUT_MS
#define NRF_BL_DFU_INACTIVITY_TIMEOUT_MS 120000
#endif

// <o> NRF_BL_FW_COPY_PROGRESS_STORE_STEP - Number of pages copied after which progress in the settings page is updated.
// <i> Progress stored in the settings page allows the bootloader to resume
// <i> copying the new firmware in case of interruption (reset).
// <i> If the value is small, then the resume point is more accurate. However,
// <i>  it also impacts negatively on flash wear.
#ifndef NRF_BL_FW_COPY_PROGRESS_STORE_STEP
#define NRF_BL_FW_COPY_PROGRESS_STORE_STEP 8
#endif

// NRF_BL_RESET_DELAY_MS - Time to wait before resetting the bootloader.
// Time (in ms) to wait before resetting the bootloader after DFU has been completed or aborted. 
// This allows more time for e.g. disconnecting the BLE link or writing logs.
#ifndef NRF_BL_RESET_DELAY_MS
#define NRF_BL_RESET_DELAY_MS 0
#endif

// <q> NRF_DFU_PROTOCOL_FW_VERSION_MSG  - Firmware version message support.
// <i> Firmware version message support.
// <i> If disabled, firmware version requests will return NRF_DFU_RES_CODE_OP_CODE_NOT_SUPPORTED.
#ifndef NRF_DFU_PROTOCOL_FW_VERSION_MSG
#define NRF_DFU_PROTOCOL_FW_VERSION_MSG 1
#endif

// <q> NRF_DFU_PROTOCOL_REDUCED  - Reduced protocol opcode selection.
// <i> Only support a minimal set of opcodes; return NRF_DFU_RES_CODE_OP_CODE_NOT_SUPPORTED
// <i> for unsupported opcodes. The supported opcodes are:NRF_DFU_OP_OBJECT_CREATE,
// <i> NRF_DFU_OP_OBJECT_EXECUTE, NRF_DFU_OP_OBJECT_SELECT, NRF_DFU_OP_OBJECT_WRITE,
// <i> NRF_DFU_OP_CRC_GET, NRF_DFU_OP_RECEIPT_NOTIF_SET, and NRF_DFU_OP_ABORT.
// <i> This reduced feature set is used by the BLE transport to reduce flash usage.
#ifndef NRF_DFU_PROTOCOL_REDUCED
#define NRF_DFU_PROTOCOL_REDUCED 0
#endif

// <q> NRF_DFU_PROTOCOL_VERSION_MSG  - Protocol version message support.
// <i> Protocol version message support.
// <i> If disabled, protocol version requests will return NRF_DFU_RES_CODE_OP_CODE_NOT_SUPPORTED.
#ifndef NRF_DFU_PROTOCOL_VERSION_MSG
#define NRF_DFU_PROTOCOL_VERSION_MSG 1
#endif


// <q> NRF_DFU_SAVE_PROGRESS_IN_FLASH  - Save DFU progress in flash.
// <i> Save DFU progress to flash so that it can be resumed if interrupted, instead of being restarted.
// <i> Keep this setting disabled to maximize transfer speed and minimize flash wear.
// <i> The init packet is always saved in flash, regardless of this setting.
#ifndef NRF_DFU_SAVE_PROGRESS_IN_FLASH
#define NRF_DFU_SAVE_PROGRESS_IN_FLASH 0
#endif

#endif //NK_SDK_CONFIG_H

