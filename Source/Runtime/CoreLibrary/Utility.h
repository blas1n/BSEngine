#pragma once

#include <type_traits>

namespace BE
{
	template <class T>
	constexpr decltype(auto) Move(T&& arg) noexcept
	{
		return static_cast<std::remove_reference_t<T>&&>(arg);
	}

	template <class T>
	constexpr T&& Forward(std::remove_reference_t<T>& arg) noexcept
	{
		return static_cast<T&&>(arg);
	}

	template <class T>
	constexpr T&& forward(std::remove_reference_t<T>&& arg) noexcept
	{
		return static_cast<T&&>(arg);
	}
}