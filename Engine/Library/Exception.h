#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	class BS_API Exception
	{
	public:
		constexpr Exception() noexcept
			: message{ TEXT("") }, needFree{ false } {}

		Exception(Char* inMessage) noexcept;
		Exception(Char* inMessage, int) noexcept;

		Exception(const Exception& other) noexcept;
		Exception(Exception&& other) noexcept;

		Exception& operator=(const Exception& other) noexcept;
		Exception& operator=(Exception&& other) noexcept;

		virtual ~Exception();

		constexpr const Char* GetMessage() const noexcept { return message; }

	private:
		void DeepCopy(Char* inMessage);

		Char* message;
		bool needFree;
	};
}