#pragma once

#include "Macro.h"

namespace BE
{
	class BS_API ComponentMemory final
	{
	public:
		constexpr ComponentMemory() noexcept {}

		void Init(void* const inMemory, const size_t inSize) noexcept {}

		template <class ComponentType>
		ComponentMemory* Allocate() { return nullptr; }

		template <class ComponentType>
		void Deallocate(ComponentType* const ptr) {}
	};
}