#include "OneFrameMemory.h"
#include <cstdlib>

namespace BE
{
	void OneFrameMemory::Init(const size_t inSize)
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
		size = inSize;
	}

	void OneFrameMemory::Release() noexcept
	{
		std::free(startMemory);
	}

	void* OneFrameMemory::Allocate(const size_t size)
	{
		if (size == 0) throw InvalidArgumentException{ };

		std::lock_guard<std::mutex> lock{ mutex };

		if (curMemory[idx] + size > startMemory[idx] + size)
			throw OutOfMemoryException{ };

		auto tmp{ curMemory[idx] };
		curMemory[idx] += size;
		return static_cast<void*>(tmp);
	}
}