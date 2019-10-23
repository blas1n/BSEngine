#pragma once

#include "PoolMemory.h"

/**
 * @brief Pool memory that allocation unit is 1
 * @detail Pool memory is designed for general purpose, so pool memory that allocation unit of 1 is the universal heap allocator.
*/
using HeapMemory = PoolMemory<1>;