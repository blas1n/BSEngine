#pragma once

#include "Core.h"

namespace BE
{
	class BS_API HeapMemory final
	{
	public:
		void Init(size_t inSize) noexcept;

		void Release() noexcept;

		void* Allocate(size_t size);

		void Deallocate(void* ptr, size_t size);

	private:
		Uint8* curMemory;
		Uint8* marker;

		size_t curNum;
		size_t maxNum;
	};
}