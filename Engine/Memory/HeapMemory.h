#pragma once

#include "Core.h"
#include <mutex>

namespace BE
{
	class BS_API HeapMemory final
	{
	public:
		void Init(SizeType inSize);
		void Release() noexcept;

		void* Allocate(SizeType size);
		void Deallocate(void* ptr, SizeType size);

	private:
		Uint8* curMemory;
		Uint8* marker;

		SizeType curNum;
		SizeType maxNum;

		std::mutex mutex;
	};
}