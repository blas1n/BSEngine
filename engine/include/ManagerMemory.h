#pragma once

#include "Core.h"

namespace BE
{
	class BS_API ManagerMemory final {
	public:
		constexpr ManagerMemory() noexcept
			: memory(nullptr), maxSize(0) {}

		inline void Init(void* const inMemory, const size_t inSize) noexcept
		{
			memory = static_cast<Uint8*>(inMemory);
			maxSize = inSize;
		}

		template <class ManagerType>
		ManagerType* Allocate() noexcept
		{
			check(memory + sizeof(ManagerType) > memory + maxSize);

			auto tmp{ memory };
			memory += sizeof(ManagerType);
			return tmp;
		}

	private:
		Uint8* memory;
		size_t maxSize;
	};
}