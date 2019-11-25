#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	class BS_API Exception
	{
	public:
		constexpr Exception(const Char* inMessage)
			: message{ inMessage } {}

		constexpr const Char* GetMessage() const noexcept
		{
			return message;
		}

	private:
		const Char* message;
	};
}