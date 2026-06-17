#ifndef DEFINES_H_
#define DEFINES_H_

#include <stdint.h>

typedef void VOID;
typedef uint32_t UINT;

#define WINAPI __attribute__((stdcall))

typedef void* HANDLE;

typedef int16_t SHORT;

typedef uint32_t DWORD;
typedef uint32_t* LPDWORD;
typedef const char* LPCTSTR;


typedef struct _COORD {
    SHORT X;
    SHORT Y;
} COORD, *PCOORD;

#endif
