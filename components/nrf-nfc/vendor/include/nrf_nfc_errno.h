/*
 * Copyright (c) 2021 Nordic Semiconductor ASA
 *
 * SPDX-License-Identifier: LicenseRef-Nordic-5-Clause
 */

#ifndef NRF_NFC_ERRNO_H__
#define NRF_NFC_ERRNO_H__

/**
 * @defgroup nrf_nfc_errno Error codes for nRF NFC libraries.
 * @{
 *
 * @brief Defines error codes that can be used in nRF NFC libraries.
 */


#ifdef __cplusplus
extern "C" {
#endif

#define NRF_EPERM            1  /**< Operation not permitted */
#define NRF_ENOENT           2  /**< No such file or directory */
#define NRF_EIO              5  /**< Input/output error */
#define NRF_ENOEXEC          8  /**< Exec format error */
#define NRF_EBADF            9  /**< Bad file descriptor */
#define NRF_ENOMEM          12  /**< Cannot allocate memory */
#define NRF_EACCES          13  /**< Permission denied */
#define NRF_EFAULT          14  /**< Bad address */
#define NRF_ENODEV          19  /**< No such device */
#define NRF_EINVAL          22  /**< Invalid argument */
#define NRF_EMFILE          24  /**< Too many open files */
#define NRF_ENOSPC          28  /**< No space left on device */
#define NRF_EAGAIN          35  /**< Resource temporarily unavailable*/
#define NRF_EDOM            37  /**< Domain error */
#define NRF_EMSGSIZE        40  /**< Message too long */
#define NRF_EPROTOTYPE      41  /**< Protocol wrong type for socket */
#define NRF_ENOPROTOOPT     42  /**< Protocol not available */
#define NRF_EPROTONOSUPPORT 43  /**< Protocol not supported */
#define NRF_ESOCKTNOSUPPORT 44  /**< Socket type not supported */
#define NRF_EOPNOTSUPP      45  /**< Operation not supported */
#define NRF_EAFNOSUPPORT    47  /**< Address family not supported by protocol */
#define NRF_EADDRINUSE      48  /**< Address already in use */
#define NRF_ENETDOWN        50  /**< Network is down */
#define NRF_ENETUNREACH     51  /**< Network is unreachable */
#define NRF_ENETRESET       52  /**< Connection aborted by network */
#define NRF_ECONNRESET      54  /**< Connection reset by peer */
#define NRF_EISCONN         56  /**< Transport endpoint is already connected */
#define NRF_ENOTCONN        57  /**< Transport endpoint is not connected */
#define NRF_ETIMEDOUT       60  /**< Connection timed out */
#define NRF_EBADMSG         77  /**< Bad message */
#define NRF_ENOBUFS         105 /**< No buffer space available */

#define NRF_EHOSTDOWN       112 /**< Host is down */
#define NRF_EALREADY        114 /**< Operation already in progress */
#define NRF_EINPROGRESS     115 /**< Operation in progress */
#define NRF_ECANCELED       125 /**< Operation canceled */

#define NRF_ENOKEY          126 /**< Required key not available */
#define NRF_EKEYEXPIRED     127 /**< Key has expired */
#define NRF_EKEYREVOKED     128 /**< Key has been revoked */
#define NRF_EKEYREJECTED    129 /**< Key was rejected by service */

#ifdef __cplusplus
}
#endif

/**
  @}
*/

#endif // NRF_NFC_ERRNO_H__
