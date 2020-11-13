#pragma once

#include "Core.h"

namespace BE
{
	class BS_API ResourceMemory final
	{
	public:
		void Init(const SizeType inSize) {}

		void Release() noexcept {}

		void* Allocate(const SizeType size) { return nullptr; }

		void Deallocate(void* const ptr, const SizeType size) {}
	};
}