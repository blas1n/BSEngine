#pragma once

#include "Core.h"
#include <type_traits>

namespace BE
{
	template <class ValueType, ValueType V>
	struct BS_API IntegralConstant
	{
		static constexpr ValueType Value = V;

		constexpr operator ValueType() const noexcept
		{
			return Value;
		}

		constexpr ValueType operator()() const noexcept
		{
			return Value;
		}
	};

	template <class ValueType, ValueType V>
	constexpr ValueType IntegralConstantValue = IntegralConstant<ValueType, V>::Value;

	template <bool B>
	using BoolConstant = IntegralConstant<bool, B>;

	using TrueType = BoolConstant<true>;
	using FalseType = BoolConstant<false>;
}