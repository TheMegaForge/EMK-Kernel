#include <stdbool.h>
#include "../defines.h"

__declspec(dllexport) bool WINAPI WriteConsoleOutputCharacterA(
    HANDLE hConsoleOutput,
    LPCTSTR lpCharacter,
    DWORD nLength,
    COORD dwWriteCoord,
    LPDWORD lpNumberOfCharsWritten
) {
    return true;
}
