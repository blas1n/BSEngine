#pragma once

#include <type_traits>

namespace BE
{
	template <class T>
	constexpr void Swap(T& a, T& b) noexcept(noexcept(std::swap(a, b)))
	{
		std::swap(a, b);
	}

	template <class T, std::size_t N>
	constexpr void Swap(T(&a)[N], T(&b)[N]) noexcept(noexcept(Swap(*a, *b)))
	{
		std::swap(a, b);
	}
}