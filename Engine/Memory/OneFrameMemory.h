#pragma once

#include "Core.h"
#include <cstdlib>

namespace BE
{
	class BS_API OneFrameMemory final {
	public:
		inline void Init(const size_t inSize)
		{
			auto ptr = std::malloc(inSize * 2);
			if (ptr == nullptr)
			{
				throw BadAllocException
				{
					TEXT("Memory required for one frame memory cannot be allocated."),
					Exception::MessageType::Shallow
				};
			}

			startMemory[0] = curMemory[0] = static_cast<Uint8*>(ptr);
			startMemory[1] = curMemory[1] = static_cast<Uint8*>(ptr) + inSize;
			maxSize = inSize;
		}

		inline void Update() noexcept
		{
			idx = (idx + 1) % 2;
			curMemory[idx] = startMemory[idx];
		}

		inline void Release() noexcept
		{
			std::free(startMemory);
		}

		void* Allocate(const size_t size)
		{
			if (curMemory[idx] + size > startMemory[idx] + maxSize)
				throw OutOfMemoryException{ };

			auto tmp{ curMemory[idx] };
			curMemory[idx] += size;
			return static_cast<void*>(tmp);
		}

	private:
		Uint8* startMemory[2];
		Uint8* curMemory[2];
		size_t maxSize;
		size_t idx;
	};
}