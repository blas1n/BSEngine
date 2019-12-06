#pragma once

#include "Core.h"
#include <mutex>

namespace BE
{
	class BS_API OneFrameMemory final {
	public:
		void Init(SizeType inSize);
		void Release() noexcept;

		inline void Update() noexcept
		{
			idx = (idx + 1) % 2;
			curMemory[idx] = startMemory[idx];
		}

		void* Allocate(SizeType size);

	private:
		Uint8* startMemory[2];
		Uint8* curMemory[2];
		SizeType size;
		SizeType idx;
		std::mutex mutex;
	};
}