#pragma once

#include "Core.h"
#include <functional>

#define CREATE_COMPARISON_STRUCT(name, oper) \
namespace BE \
{ \
	template <class T = void> \
	struct BS_API name final \
	{ \
		constexpr bool operator()(const T& lhs, const T& rhs) const noexcept(noexcept(lhs oper rhs)) \
		{ \
			return lhs oper rhs; \
		} \
	}; \
\
	template <> \
	struct BS_API name<void> final \
	{ \
		template <class T, class U> \
		constexpr bool operator()(const T&& lhs, const U&& rhs) const \
			noexcept(noexcept(std::forward<T>(lhs) oper std::forward<U>(rhs))) \
		{ \
			return Forward<T>(lhs) oper Forward<U>(rhs); \
		} \
	}; \
}

CREATE_COMPARISON_STRUCT(Equal, ==)
CREATE_COMPARISON_STRUCT(NotEqual, !=)
CREATE_COMPARISON_STRUCT(Less, <)
CREATE_COMPARISON_STRUCT(Greater, >)
CREATE_COMPARISON_STRUCT(LessEqual, <=)
CREATE_COMPARISON_STRUCT(GreatEqual, >=)