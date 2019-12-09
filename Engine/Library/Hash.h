#pragma once

#include "Core.h"
#include <functional>

namespace BE
{
	template <class T>
	struct BS_API Hash final
	{
		SizeType operator()(const T& value) const noexcept
		{
			return std::hash<T>{}(value);
		}
	};
}