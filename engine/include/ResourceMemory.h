#pragma once

#include "Core.h"

namespace BE
{
	class BS_API ResourceMemory final
	{
	public:
		constexpr ResourceMemory() noexcept {}

		void Init(void* const inMemory, const size_t inSize) noexcept {}

		void* Allocate(const size_t size) { return nullptr; }

		void Deallocate(void* const ptr, const size_t size) {}
	};
}