#pragma once

#include "Core.h"

namespace BE
{
	class BS_API OneFrameMemory final {
	public:
		constexpr OneFrameMemory() noexcept
			: curMemory(nullptr),
			startMemory(nullptr),
			maxSize(0) {}

		inline void Init(void* const inMemory, const size_t inSize) noexcept
		{
			curMemory = startMemory = static_cast<Uint8*>(inMemory);
			maxSize = inSize;
		}

		void* Allocate(const size_t size)
		{
			if (curMemory + size > curMemory + maxSize)
				throw Exception(TEXT("Can not allocate one frame memory!"));

			auto tmp{ curMemory };
			curMemory += size;
			return tmp;
		}

		inline void Clear() noexcept
		{
			curMemory = startMemory;
		}

	private:
		Uint8* curMemory;
		Uint8* startMemory;
		size_t maxSize;
	};
}