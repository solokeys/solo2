MEMORY
{

  /* musca-b1 does not seem to start in secure mode, */
  /* can't use 0x0 FLASH here                        */
  FLASH : ORIGIN = 0x10000000, LENGTH = 128K
  RAM :   ORIGIN = 0x30000000, LENGTH = 32K

}
