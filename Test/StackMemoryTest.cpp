#include "pch.h"
#include "StackMemory.h"

TEST(StackMemory, Create)
{
	StackMemory invalidStack{ 0 };
	ASSERT_EQ(invalidStack.GetAssignedByte(), 0);
	ASSERT_EQ(invalidStack.GetAssignableByte(), 0);
	ASSERT_EQ(invalidStack.GetMaxByte(), 0);

	StackMemory stack{ 12 };
	ASSERT_EQ(stack.GetAssignedByte(), 0);
	ASSERT_EQ(stack.GetAssignableByte(), 12);
	ASSERT_EQ(stack.GetMaxByte(), 12);
}

TEST(StackMemory, Malloc)
{
	StackMemory stack{ 5 };
	auto p = stack.Malloc(6);
	ASSERT_EQ(p, nullptr);

	p = stack.Malloc(3);
	ASSERT_EQ(stack.GetAssignedByte(), 3);

	p = stack.Malloc(3);
	ASSERT_EQ(p, nullptr);

	p = stack.Malloc(2);
	ASSERT_EQ(stack.GetAssignableByte(), 0);
}

TEST(StackMemory, Free)
{
	StackMemory stack{ 5 };

	auto p = stack.Malloc(5);
	stack.Free(p);
	ASSERT_EQ(stack.GetAssignedByte(), 0);

	auto a = stack.Malloc(2);
	auto b = stack.Malloc(1);
	auto c = stack.Malloc(2);

	stack.Free(b);
	ASSERT_EQ(stack.GetAssignableByte(), 3);

	stack.Free(a);
	ASSERT_EQ(stack.GetAssignedByte(), 0);
}

TEST(StackMemory, FreeWithInvalidArgument)
{
	StackMemory stack{ 5 };
	auto p = static_cast<unsigned char*>(stack.Malloc(1));

#if _DEBUG
	ASSERT_DEBUG_DEATH(stack.Free(p - 1), ".*Assert.*");
	ASSERT_DEBUG_DEATH(stack.Free(p + 1), ".*Assert.*");
	ASSERT_DEBUG_DEATH(stack.Free(nullptr), ".*Assert.*");
#else
	pool.Free(p - 1);
	pool.Free(p + 1);
	pool.Free(nullptr);
#endif
}

TEST(StackMemory, FreeWithInvalidSequence)
{
	StackMemory stack{ 5 };
	auto a = stack.Malloc(2);
	auto b = stack.Malloc(1);
	stack.Free(a);

#if _DEBUG
	ASSERT_DEBUG_DEATH(stack.Free(b), ".*Assert.*");
#else
	pool.Free(b);
#endif
}

TEST(StackMemory, ReadWrite)
{
	StackMemory stack{ 10 };

	auto a = static_cast<int*>(stack.Malloc(4));
	*a = rand() % std::numeric_limits<int>::max();
	auto b = static_cast<short*>(stack.Malloc(2));
	*b = rand() % std::numeric_limits<short>::max();

	stack.Free(b);
	auto c = static_cast<int*>(stack.Malloc(4));
	ASSERT_EQ(*c, 0);

	stack.Free(a);
	auto d = static_cast<char*>(stack.Malloc(3));
	d[0] = 'a';
	d[1] = 'b';
	d[2] = 0;
	
	ASSERT_STREQ(d, "ab");
}