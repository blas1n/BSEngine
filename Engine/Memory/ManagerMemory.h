#pragma once

#include "Core.h"
#include <cstdlib>

namespace BE
{
	class BS_API ManagerMemory final {
	public:
		inline void Init(const SizeType inSize)
		{
			startMemory = curMemory = static_cast<Uint8*>(std::malloc(inSize));
			if (startMemory == nullptr)
			{
				throw BadAllocException
				{
					TEXT("Memory required for manager memory cannot be allocated."),
					Exception::MessageType::Shallow
				};
			}

			size = inSize;
		}

		inline void Release() noexcept
		{
			std::free(startMemory);
		}

		template <class ManagerType>
		ManagerType* Allocate()
		{
			if (curMemory + size > startMemory + size)
				throw OutOfMemoryException{ };

			auto tmp{ curMemory };
			curMemory += sizeof(ManagerType);
			return reinterpret_cast<ManagerType*>(tmp);
		}

	private:
		Uint8* startMemory;
		Uint8* curMemory;
		SizeType size;
	};
}