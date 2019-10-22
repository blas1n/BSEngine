#pragma once

#include "IMemory.h"
#include <limits>

class BS_API StackMemory final : public IMemory
{
public:
	StackMemory(size_t size) noexcept;
	~StackMemory();

	void* Malloc(size_t size) override;

	// note : If you try to free a pointer that is not at the top, all the pointers above it are freed.
	void Free(void* ptr) override;

	void FreeAll();

private:
	using byte = unsigned char;
	using dbyte = unsigned short;

	static constexpr dbyte MAX_SIZE = std::numeric_limits<dbyte>::max();

	byte* cur;
	byte* start;
	byte* end;
};