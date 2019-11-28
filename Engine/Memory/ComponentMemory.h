#pragma once

#include "Core.h"

namespace BE
{
	class BS_API ComponentMemory final
	{
	public:
		void Init(const size_t inSize) {}
		
		void Release() noexcept {}

		template <class ComponentType>
		ComponentMemory* Allocate() { return nullptr; }

		template <class ComponentType>
		void Deallocate(ComponentType* const ptr) {}
	};
}