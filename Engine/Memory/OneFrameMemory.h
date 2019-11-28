#pragma once

#include "Core.h"
#include <mutex>

namespace BE
{
	class BS_API OneFrameMemory final {
	public:
		void Init(size_t inSize);
		void Release() noexcept;

		inline void Update() noexcept
		{
			idx = (idx + 1) % 2;
			curMemory[idx] = startMemory[idx];
		}

		void* Allocate(size_t size);

	private:
		Uint8* startMemory[2];
		Uint8* curMemory[2];
		size_t size;
		size_t idx;
		std::mutex mutex;
	};
}