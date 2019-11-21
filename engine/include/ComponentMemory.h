#pragma once

#include "Macro.h"

namespace BE
{
	class BS_API ComponentMemory final
	{
	public:
		constexpr ComponentMemory() noexcept {}

		void Init(const size_t inSize) noexcept {}
		
		void Release() noexcept {}

		template <class ComponentType>
		ComponentMemory* Allocate() { return nullptr; }

		template <class ComponentType>
		void Deallocate(ComponentType* const ptr) {}
	};
}