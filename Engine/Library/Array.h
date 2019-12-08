#pragma once

#include "Core.h"
#include <vector>
#include <algorithm>

namespace BE
{
	template <class T, class Allocator>
	class BS_API Array final
	{
	private:
		using ContainerType = std::vector<T, Allocator>;

	public:
		using ElementType = T;
		using AllocatorType = Allocator<T>;

		Array() noexcept = default;
		Array(const Array& other) = default;
		Array(Array&& other) noexcept = default;

		Array(const T* ptr, SizeType count) : container(ptr, count) {}
		Array(std::initializer_list<T> elems) : container(elems) {}
		
		template <class OtherElement>
		explicit Array(const Array<OtherElement, Allocator>& other);

		template <class OtherElement>
		explicit Array(Array<OtherElement, Allocator>&& other);

		~Array() = default;

		inline Array& operator=(const Array& other)
		{
			container = other.container;
			return *this;
		}

		inline Array& operator=(Array&& other) noexcept
		{
			container = std::move(other.container);
			return *this;
		}

		inline Array& operator=(std::initializer_list<T> elems)
		{
			container = elems;
			return *this;
		}

		inline T& operator[](SizeType index) { return container.at(index); }
		inline const T& operator[](SizeType index) const { return container.at(index); }

		void Insert(const Array& other, SizeType pos);
		void Insert(Array&& other, SizeType pos);

		void Insert(const T& item, SizeType pos);
		void Insert(T&& item, SizeType pos);

		void Insert(std::initializer_list<T> elems, SizeType pos);
		void Insert(T* ptr, SizeType count, SizeType pos);

		inline void Add(const T& item) { Insert(item, GetSize()); }
		inline void Add(T&& item) { Insert(item, GetSize()); }

		void AddUnique(const T& item);
		void AddUnique(T&& item);

		inline void Append(std::initializer_list<T> elems) { Insert(elems, GetSize()); }
		inline void Append(const T* ptr, SizeType count) { Insert(ptr, count, GetSize()); }

		inline void Append(const Array& other) { Insert(other, GetSize()); }
		inline void Append(Array&& other) { Insert(std::move(other), GetSize()); }
	
		template <class... Args>
		inline void Emplace(Args&&... args)
		{
			container.emplace_back(std::forward<Args>(args)...);
		}

		template <class... Args>
		inline void EmplaceAt(SizeType pos, Args&&... args)
		{
			container.emplace(container.cbegin() + pos, std::forward<Args>(args)...);
		}

		void Remove(const T& item);
		void RemoveLast(const T& item);

		SizeType RemoveAll(const T& item) noexcept;

		void RemoveAt(SizeType index, SizeType count = 1);

		template <class Predicate>
		SizeType RemoveByPredicate(Predicate&& pred) noexcept;

		bool Find(const T& value, SizeType& index) const noexcept;
		SizeType Find(const T& value) const noexcept;

		bool FindLast(const T& value, SizeType& index) const noexcept;
		SizeType FindLast(const T& value) const noexcept;

		Array<T&, Allocator> FindAll(const T& value) noexcept;
		Array<const T&, Allocator> FindAll(const T& value) const noexcept;

		template <class Pred>
		bool Find(Pred&& pred, SizeType& index) const noexcept;

		template <class Pred>
		SizeType Find(Pred&& pred) const noexcept;

		template <class Pred>
		bool FindLast(Pred&& pred, SizeType& index) const noexcept;

		template <class Pred>
		SizeType FindLast(Pred&& pred) const noexcept;

		template <class Pred>
		Array<T&, Allocator> FindAll(Pred&& pred) noexcept;

		template <class Pred>
		Array<const T&, Allocator> FindAll(Pred&& pred) const noexcept;

		inline void Resize(SizeType size, const T& value = T()) { container.resize(size, value); }
		inline void Reserve(SizeType size) { container.reserve(size); }

		inline bool IsEmpty() const noexcept { return container.empty(); }
		
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetCapacity() const noexcept { return container.capacity(); }

		inline void Shrink() noexcept { container.shrink_to_fit(); }

		inline void Clear() noexcept { container.clear(); }

		// Don't use! Only range based for for the function.
		inline typename ContainerType::iterator begin() noexcept { return container.begin(); }
		inline typename ContainerType::const_iterator begin() const noexcept { return container.begin(); }

		inline typename ContainerType::iterator end() noexcept { return container.end(); }
		inline typename ContainerType::const_iterator end() const noexcept { return container.end(); }

	private:
		friend bool operator==(const Array& lhs, const Array& rhs) noexcept;

		ContainerType container;
	};

	template <class T, class Allocator>
	bool operator==(const Array<T, Allocator>& lhs, const Array<T, Allocator>& rhs)
	{
		return lhs.container == rhs.container;
	}

	template <class T, class Allocator>
	bool operator!=(const Array<T, Allocator>& lhs, const Array<T, Allocator>& rhs)
	{
		return !(lhs == rhs);
	}

	template <class T, class Allocator>
	template <class OtherElement>
	Array<T, Allocator>::Array(const Array<OtherElement, Allocator>& other)
	{
		Reserve(other.GetSize());
		for (auto& elem : other)
			Emplace(elem);
	}

	template <class T, class Allocator>
	template <class OtherElement>
	Array<T, Allocator>::Array(Array<OtherElement, Allocator>&& other)
	{
		Reserve(other.GetSize());
		for (auto&& elem : std::move(other))
			Emplace(std::move(elem));
	}

	template <class T, class Allocator>
	bool Array<T, Allocator>::Find(const T& value, SizeType& index) const noexcept
	{
		index = Find(value);
		return index == GetSize() + 1;
	}

	template <class T, class Allocator>
	SizeType Array<T, Allocator>::Find(const T& value) const noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();
		return std::find(begin, end, value) - begin;
	}

	template <class T, class Allocator>
	bool Array<T, Allocator>::FindLast(const T& value, SizeType& index) const noexcept
	{
		index = FindLast(value);
		return index == GetSize() + 1;
	}

	template <class T, class Allocator>
	SizeType Array<T, Allocator>::FindLast(const T& value) const noexcept
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();
		return std::find(begin, end, value) - begin;
	}

	template <class T, class Allocator>
	Array<T&, Allocator> Array<T, Allocator>::FindAll(const T& value) noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, value);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, class Allocator>
	Array<const T&, Allocator> Array<T, Allocator>::FindAll(const T& value) const noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, value);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, class Allocator>
	template <class Pred>
	bool Array<T, Allocator>::Find(Pred&& pred, SizeType& index) const noexcept
	{
		index = Find(std::forward<Pred>(pred));
		return index == GetSize() + 1;
	}

	template <class T, class Allocator>
	template <class Pred>
	SizeType Array<T, Allocator>::Find(Pred&& pred) const noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();
		return std::find_if(begin, end, pred) - begin;
	}

	template <class T, class Allocator>
	template <class Pred>
	bool Array<T, Allocator>::FindLast(Pred&& pred, SizeType& index) const noexcept
	{
		index = FindLast(std::forward<Pred>(pred));
		return index == GetSize() + 1;
	}

	template <class T, class Allocator>
	template <class Pred>
	SizeType Array<T, Allocator>::FindLast(Pred&& pred) const noexcept
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();
		return std::find_if(begin, end, pred) - begin;
	}

	template <class T, class Allocator>
	template <class Pred>
	Array<T&, Allocator> Array<T, Allocator>::FindAll(Pred&& pred) noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, pred);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, class Allocator>
	template <class Pred>
	Array<const T&, Allocator> Array<T, Allocator>::FindAll(Pred&& pred) const noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find_if(iter, end, pred);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}
}