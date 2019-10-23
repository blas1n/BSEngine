#pragma once

#include "IMemory.h"
#include <vector>

/**
 * @brief Memory with fixed allocation size
*/
template <size_t Size>
class BS_API PoolMemory : public IMemory {
public:
	PoolMemory(size_t count) noexcept;
	~PoolMemory();

	void* Malloc(size_t count = 1) override;

	void Free(void* ptr) override;

	void Clear() override;

private:
	using byte = unsigned char;

	byte* memory;
	size_t* marker;

	size_t curNum;
	size_t maxNum;
};