# Re-build the vendored static libraries + bindgen output for the
# nrf-nfc crate. Normal builds don't need this — `cargo build` reads
# the committed artifacts in vendor/ directly.
#
# Run `make regen` when bumping nrfx (the chip layer's source) or when
# Nordic ships a new libnfc_t4t.a (the closed-source blob in
# vendor/lib/libnfc_t4t.a — copy it in by hand from upstream nrfxlib
# first, then run regen to refresh the bindgen output).
#
# Requires: arm-none-eabi-gcc (in PATH) + bindgen-cli
# (cargo install bindgen-cli).

# Pinned commits for the upstream sources we regenerate libnrfx_nfct.a
# from. Bump these together when upgrading the nrfx stack.
NRFX_COMMIT  := aa83d4df8d5f41b591f23a8555794632afb3475d
CMSIS_COMMIT := f6226079fdb207e945dae7cea2c273ea5200376a

VENDOR := vendor
BUILD  := .regen-build

.PHONY: regen regen-clean

regen: regen-nrfx regen-t4t

regen-clean:
	rm -rf $(BUILD)

# ── Embedded glue header bodies ──────────────────────────────────────────────
#
# These four headers are project-specific config nrfx requires at C
# compile + bindgen time. They live here (not in the crate tree)
# because the crate tree should contain only what the *normal* build
# needs, which doesn't include any C compilation. The regen target
# materializes them in $(BUILD)/glue, uses them, then leaves them for
# the next regen run (gitignored).

define NRFX_CONFIG_H_BODY
#ifndef NRFX_CONFIG_H__
#define NRFX_CONFIG_H__
#ifndef NRF52840_XXAA
#define NRF52840_XXAA
#endif
#define NRFX_NFCT_ENABLED                       1
#define NRFX_NFCT_CONFIG_TIMER_INSTANCE_ID      4
#define NRFX_NFCT_CONFIG_IRQ_PRIORITY           6
#define NRFX_NFCT_CONFIG_LOG_ENABLED            0
#define NRFX_TIMER_ENABLED                      1
#define NRFX_TIMER4_ENABLED                     1
#define NRFX_TIMER_DEFAULT_CONFIG_FREQUENCY     0
#define NRFX_TIMER_DEFAULT_CONFIG_MODE          0
#define NRFX_TIMER_DEFAULT_CONFIG_BIT_WIDTH     0
#define NRFX_TIMER_DEFAULT_CONFIG_IRQ_PRIORITY  6
#define NRFX_TIMER_CONFIG_LOG_ENABLED           0
#include <nrfx_config_common.h>
#endif
endef
export NRFX_CONFIG_H_BODY

define NRFX_GLUE_H_BODY
#ifndef NRFX_GLUE_H__
#define NRFX_GLUE_H__
#include <nrfx.h>
#define NRFX_ASSERT(e)                  ((void)(e))
#define NRFX_STATIC_ASSERT(e)           _Static_assert(e, #e)
#define NRFX_IRQ_PRIORITY_SET(i, p)     NVIC_SetPriority((IRQn_Type)(i), p)
#define NRFX_IRQ_ENABLE(i)              NVIC_EnableIRQ((IRQn_Type)(i))
#define NRFX_IRQ_IS_ENABLED(i)          (NVIC_GetEnableIRQ((IRQn_Type)(i)) != 0)
#define NRFX_IRQ_DISABLE(i)             NVIC_DisableIRQ((IRQn_Type)(i))
#define NRFX_IRQ_PENDING_SET(i)         NVIC_SetPendingIRQ((IRQn_Type)(i))
#define NRFX_IRQ_PENDING_CLEAR(i)       NVIC_ClearPendingIRQ((IRQn_Type)(i))
#define NRFX_IRQ_IS_PENDING(i)          (NVIC_GetPendingIRQ((IRQn_Type)(i)) != 0)
extern uint8_t nrfx_critical_section_enter(void);
extern void    nrfx_critical_section_exit(uint8_t saved);
#define NRFX_CRITICAL_SECTION_ENTER()   { uint8_t _nrfx_cs = nrfx_critical_section_enter();
#define NRFX_CRITICAL_SECTION_EXIT()      nrfx_critical_section_exit(_nrfx_cs); }
extern void nrfx_delay_us(uint32_t us);
#define NRFX_DELAY_US(us)               nrfx_delay_us(us)
#define NRFX_ATOMIC_FETCH_STORE(p, v)   __atomic_exchange_n(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_FETCH_OR(p, v)      __atomic_fetch_or(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_FETCH_AND(p, v)     __atomic_fetch_and(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_FETCH_XOR(p, v)     __atomic_fetch_xor(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_FETCH_ADD(p, v)     __atomic_fetch_add(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_FETCH_SUB(p, v)     __atomic_fetch_sub(p, v, __ATOMIC_SEQ_CST)
#define NRFX_ATOMIC_CAS(p, ov, nv) ({ __typeof__(ov) _o = (ov); __atomic_compare_exchange_n(p, &_o, nv, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST); })
#define NRFX_CLZ(v)                     __builtin_clz(v)
#define NRFX_CTZ(v)                     __builtin_ctz(v)
#define NRFX_POPCOUNT(v)                __builtin_popcount(v)
#define NRFX_EVENT_READBACK_ENABLED     1
#define NRFX_DPPI_CHANNELS_USED         0
#define NRFX_DPPI_GROUPS_USED           0
#define NRFX_PPI_CHANNELS_USED          0
#define NRFX_PPI_GROUPS_USED            0
#define NRFX_GPIOTE_CHANNELS_USED       0
#define NRFX_EGUS_USED                  0
#define NRFX_TIMERS_USED                0
#define NRFX_CUSTOM_ERROR_CODES         0
#endif
endef
export NRFX_GLUE_H_BODY

define NRFX_LOG_H_BODY
#ifndef NRFX_LOG_H__
#define NRFX_LOG_H__
#define NRFX_LOG_INFO(...)              ((void)0)
#define NRFX_LOG_WARNING(...)           ((void)0)
#define NRFX_LOG_ERROR(...)             ((void)0)
#define NRFX_LOG_DEBUG(...)             ((void)0)
#define NRFX_LOG_HEXDUMP_INFO(p, len)   ((void)0)
#define NRFX_LOG_HEXDUMP_WARNING(p,len) ((void)0)
#define NRFX_LOG_HEXDUMP_ERROR(p, len)  ((void)0)
#define NRFX_LOG_HEXDUMP_DEBUG(p, len)  ((void)0)
#define NRFX_LOG_ERROR_STRING_GET(c)    ""
#endif
endef
export NRFX_LOG_H_BODY

define NRFX_WRAPPER_H_BODY
#include <nrfx.h>
#include <nrfx_nfct.h>
#include <nrfx_timer.h>
endef
export NRFX_WRAPPER_H_BODY

define T4T_WRAPPER_H_BODY
#define NRF52840_XXAA
#include <nfc_t4t_lib.h>
#include <nrf_nfc_errno.h>
endef
export T4T_WRAPPER_H_BODY

# nrfx include flags reused by both compile + bindgen invocations.
NRFX_INCLUDES = \
    -I$(BUILD)/glue \
    -I$(BUILD)/cmsis/CMSIS/Core/Include \
    -I$(BUILD)/nrfx/drivers/include \
    -I$(BUILD)/nrfx/hal -I$(BUILD)/nrfx/haly \
    -I$(BUILD)/nrfx/helpers -I$(BUILD)/nrfx/templates \
    -I$(BUILD)/nrfx/bsp/stable -I$(BUILD)/nrfx/bsp/stable/mdk \
    -I$(BUILD)/nrfx/bsp/stable/soc -I$(BUILD)/nrfx/bsp/stable/soc/irqs \
    -I$(BUILD)/nrfx/bsp/stable/soc/interconnect \
    -I$(BUILD)/nrfx/bsp/stable/templates -I$(BUILD)/nrfx

# ── regen-nrfx ──────────────────────────────────────────────────────────────
#
# Compile libnrfx_nfct.a from upstream sources + regenerate
# nrfx_nfct_bindings.rs.
regen-nrfx: $(BUILD)/nrfx/.git $(BUILD)/cmsis/.git
	@command -v arm-none-eabi-gcc >/dev/null || { echo "arm-none-eabi-gcc not in PATH"; exit 1; }
	@command -v bindgen           >/dev/null || { echo "bindgen-cli not installed; run: cargo install bindgen-cli"; exit 1; }
	@mkdir -p $(BUILD)/glue
	@printf '%s\n' "$$NRFX_CONFIG_H_BODY"  > $(BUILD)/glue/nrfx_config.h
	@printf '%s\n' "$$NRFX_GLUE_H_BODY"    > $(BUILD)/glue/nrfx_glue.h
	@printf '%s\n' "$$NRFX_LOG_H_BODY"     > $(BUILD)/glue/nrfx_log.h
	@printf '%s\n' "$$NRFX_WRAPPER_H_BODY" > $(BUILD)/glue/wrapper.h
	@echo "── Compiling libnrfx_nfct.a ──────────────────────────────────────────"
	@for src in nrfx_nfct.c nrfx_timer.c; do                          \
	    arm-none-eabi-gcc -c -mthumb -mcpu=cortex-m4 -mfloat-abi=soft \
	        -DNRF52840_XXAA -D__HEAP_SIZE=0 -D__STACK_SIZE=0          \
	        -fno-builtin -fno-strict-aliasing                         \
	        -Wno-unused-parameter -Wno-expansion-to-defined -O2       \
	        $(NRFX_INCLUDES)                                          \
	        -o $(BUILD)/$${src%.c}.o                                  \
	        $(BUILD)/nrfx/drivers/src/$${src} || exit 1;              \
	done
	@arm-none-eabi-ar rcs $(VENDOR)/lib/libnrfx_nfct.a \
	    $(BUILD)/nrfx_nfct.o $(BUILD)/nrfx_timer.o
	@echo "── Generating nrfx_nfct_bindings.rs ─────────────────────────────────"
	@bindgen $(BUILD)/glue/wrapper.h                                              \
	    --use-core --ctypes-prefix cty --no-layout-tests --with-derive-default    \
	    --allowlist-function 'nrfx_nfct_.*' --allowlist-function 'nrf_nfct_.*'    \
	    --allowlist-type     'nrfx_nfct_.*' --allowlist-type     'nrf_nfct_.*'    \
	    --allowlist-var      'NRFX_NFCT_.*' --allowlist-var      'NRF_NFCT_.*'    \
	    --output $(VENDOR)/nrfx_nfct_bindings.rs                                  \
	    -- --target=thumbv7em-none-eabi -mthumb -mcpu=cortex-m4 -DNRF52840_XXAA   \
	    -isystem "$$(arm-none-eabi-gcc -print-sysroot)/include"                   \
	    $(NRFX_INCLUDES)

# ── regen-t4t ───────────────────────────────────────────────────────────────
#
# Re-generate nfc_t4t_bindings.rs. The .a is Nordic's closed-source
# blob — copy a new vendor/lib/libnfc_t4t.a from upstream nrfxlib by
# hand before running this if you're bumping the version.
regen-t4t:
	@command -v arm-none-eabi-gcc >/dev/null || { echo "arm-none-eabi-gcc not in PATH"; exit 1; }
	@command -v bindgen           >/dev/null || { echo "bindgen-cli not installed; run: cargo install bindgen-cli"; exit 1; }
	@mkdir -p $(BUILD)
	@printf '%s\n' "$$T4T_WRAPPER_H_BODY" > $(BUILD)/t4t_wrapper.h
	@bindgen $(BUILD)/t4t_wrapper.h                                               \
	    --use-core --ctypes-prefix cty --no-layout-tests --with-derive-default    \
	    --allowlist-function 'nfc_t4t_.*'                                         \
	    --allowlist-type     'nfc_t4t_.*'                                         \
	    --allowlist-var      'NFC_T4T_.*'                                         \
	    --output $(VENDOR)/nfc_t4t_bindings.rs                                    \
	    -- --target=thumbv7em-none-eabi -mthumb -mcpu=cortex-m4 -DNRF52840_XXAA   \
	    -isystem "$$(arm-none-eabi-gcc -print-sysroot)/include"                   \
	    -I$(VENDOR)/include

$(BUILD)/nrfx/.git:
	@mkdir -p $(BUILD)
	git clone https://github.com/NordicSemiconductor/nrfx.git $(BUILD)/nrfx
	cd $(BUILD)/nrfx && git checkout $(NRFX_COMMIT)

$(BUILD)/cmsis/.git:
	@mkdir -p $(BUILD)
	git clone --filter=blob:none https://github.com/ARM-software/CMSIS_6 $(BUILD)/cmsis
	cd $(BUILD)/cmsis && git checkout $(CMSIS_COMMIT)
