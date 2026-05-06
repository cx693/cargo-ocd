/* ============================================================
 *  存储器布局配置
 *  ============================================================
 *  根据你的芯片修改 FLASH 和 RAM 的大小
 *
 *  常见 STM32 芯片配置：
 *    STM32F103C8T6  (Blue Pill):  FLASH = 64K,  RAM = 20K
 *    STM32F103CBT6:                FLASH = 128K, RAM = 20K
 *    STM32F103RCT6:                FLASH = 256K, RAM = 48K
 *    STM32F103VET6:                FLASH = 512K, RAM = 64K
 *    STM32F401CCU6 (Black Pill):   FLASH = 256K, RAM = 64K
 *    STM32F411CEU6 (Black Pill):   FLASH = 512K, RAM = 128K
 *    STM32F407VGT6 (Discovery):    FLASH = 1M,   RAM = 192K
 *    STM32F746NGH6 (Discovery):    FLASH = 1M,   RAM = 320K
 * ============================================================ */

MEMORY
{
  /* Flash 起始地址固定为 0x08000000 */
  FLASH : ORIGIN = 0x08000000, LENGTH = 64K

  /* RAM 起始地址固定为 0x20000000 */
  RAM : ORIGIN = 0x20000000, LENGTH = 20K
}

/* 栈顶指针 = RAM 起始地址 + RAM 大小 */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);
