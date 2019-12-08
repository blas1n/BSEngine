#pragma once

#include "Core.h"
#include <stack>

namespace BE
{
	template <class T, template <class> class Allocator>
	class BS_API Stack final
	{
	public:
		using ElementType = T;
		using AllocatorType = Allocator<T>;

		Stack() noexcept : container{ } { }

		Stack(const Stack& other) : container{ other.container } {}
		Stack(Stack&& other) noexcept : container{ std::move(other.container) } {}

		~Stack() = default;

		inline Stack& operator=(const Stack& other)
		{
			container = other.container;
			return *this;
		}

		inline Stack& operator=(Stack&& other) noexcept
		{
			container = std::move(other.container);
			return *this;
		}

		inline void Push(const T& value)
		{
			container.push(value);
		}

		inline void Push(T&& value)
		{
			container.push(std::move(value));
		}

		template <class... Args>
		inline void Emplace(Args&&... args)
		{
			container.emplace(std::forward<Args>(args)...);
		}

		inline void Pop()
		{
			container.pop();
		}

		inline T& Peek()
		{
			return container.top();
		}

		inline const T& Peek() const
		{
			return container.top();
		}

		inline SizeType GetSize() const noexcept
		{
			return container.size();
		}

		inline bool IsEmpty() const noexcept
		{
			return container.empty();
		}

		inline void Clear() noexcept { while (IsEmpty()) Pop(); }

	private:
		friend bool operator==(const Stack& lhs, const Stack& rhs) noexcept;

		std::stack<T, std::deque<T, Allocator<T>>> container;
	};

	template <class T, template <class> class Allocator>
	bool operator==(const Stack<T, Allocator>& lhs, const Stack<T, Allocator>& rhs)
	{
		return lhs.container == rhs.container;
	}

	template <class T, template <class> class Allocator>
	bool operator!=(const Stack<T, Allocator>& lhs, const Stack<T, Allocator>& rhs)
	{
		return !(lhs == rhs);
	}
}