/*
 * Host tests of the layout logic in our sntool's sunxi-part (hakchi fork, sntool/sunxi_part.c)
 * against the console's boot-info blocks and the shipped sntool's output on them
 * (scripts/userland/fixtures). Built and run by run.sh.
 *
 *   test_sunxi_part <fixtures dir>
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sunxi_part.h"

static int failures;

#define CHECK(cond)                                                                       \
	do {                                                                              \
		if (!(cond)) {                                                            \
			fprintf(stderr, "%s:%d: failed: %s\n", __FILE__, __LINE__, #cond); \
			failures++;                                                       \
		}                                                                         \
	} while (0)

static uint8_t *load(const char *dir, const char *name) {
	char path[1024];
	uint8_t *b = malloc(SP_INFO_SIZE);
	FILE *f;
	snprintf(path, sizeof(path), "%s/%s", dir, name);
	f = fopen(path, "rb");
	if (b == NULL || f == NULL || fread(b, 1, SP_INFO_SIZE, f) != SP_INFO_SIZE) {
		fprintf(stderr, "cannot read %s\n", path);
		exit(2);
	}
	fclose(f);
	return b;
}

static const char *format(const struct sp_layout *l) {
	static char line[1024];
	sp_format(line, sizeof(line), l);
	return line;
}

/* Renders l over a copy of base and compares with want. */
static int renders_to(const uint8_t *base, const struct sp_layout *l, const uint8_t *want) {
	uint8_t *b = malloc(SP_INFO_SIZE);
	int same;
	memcpy(b, base, SP_INFO_SIZE);
	sp_render(b, l);
	same = memcmp(b, want, SP_INFO_SIZE) == 0;
	if (!same)
		for (unsigned i = 0; i < SP_INFO_SIZE; i++)
			if (b[i] != want[i]) {
				fprintf(stderr, "  first difference at 0x%x: 0x%02x, want 0x%02x\n", i, b[i], want[i]);
				break;
			}
	free(b);
	return same;
}

int main(int argc, char *argv[]) {
	const char *dir = argc > 1 ? argv[1] : ".";
	uint8_t *stock = load(dir, "snes-eur-nandinfo-stock.bin");
	uint8_t *grown = load(dir, "snes-eur-nandinfo-grown.bin");
	uint8_t *shrunk = load(dir, "snes-eur-nandinfo-shrunk.bin");
	struct sp_layout s, g, h, out, again;

	/* Parsing and the listing the console prints. */
	CHECK(sp_parse(stock, &s) == 0);
	CHECK(sp_parse(grown, &g) == 0);
	CHECK(sp_parse(shrunk, &h) == 0);
	CHECK(strcmp(format(&g), "mbr::32768 nanda:boot:6144 nandb:rootfs:131072 nandc:data:741376") == 0);
	CHECK(strcmp(format(&s), "mbr::32768 nanda:boot:6144 nandb:rootfs:131072 nandc:data:614400 "
				 "nandd:private:2048 nande:UDISK:124928") == 0);
	CHECK(strcmp(format(&h), format(&s)) == 0);
	CHECK(s.total == 911360 && s.part[5].len == 0 && sp_size(&s, 5) == 124928);

	/* grow: factory and shrunk blocks to the grown block the shipped tool writes, byte for byte. */
	CHECK(sp_grow(&s, &out) == 0);
	CHECK(sp_equal(&out, &g));
	CHECK(renders_to(stock, &out, grown));
	CHECK(sp_grow(&h, &out) == 0);
	CHECK(renders_to(shrunk, &out, grown));
	CHECK(sp_grow(&g, &out) == 1); /* already grown */

	/* shrink: back to the factory layout from an untouched copy, as the shipped tool writes it. */
	CHECK(sp_shrink(&g, &s, &out) == 0);
	CHECK(sp_equal(&out, &h));
	CHECK(renders_to(grown, &out, shrunk));
	CHECK(sp_shrink(&s, &s, &out) == 1); /* already factory */
	CHECK(sp_shrink(&h, &s, &out) == 1);
	CHECK(sp_shrink(&g, NULL, &out) == -1); /* no factory layout */
	CHECK(sp_shrink(&g, &g, &out) == -1);   /* not a factory layout */

	/* Round trips. */
	CHECK(sp_grow(&h, &out) == 0 && sp_shrink(&out, &s, &again) == 0 && sp_equal(&again, &h));
	CHECK(renders_to(grown, &g, grown));
	CHECK(renders_to(shrunk, &h, shrunk));

	/* Refusals: damaged blocks, unknown layouts. */
	shrunk[0x300] ^= 1; /* checksum no longer matches */
	CHECK(sp_parse(shrunk, &out) == -1);
	shrunk[0x300] ^= 1;
	stock[0] = 0;
	CHECK(sp_parse(stock, &out) == -1);
	out = s;
	out.count = 5; /* no UDISK */
	CHECK(sp_grow(&out, &again) == -1);
	out = s;
	strcpy(out.part[4].name, "other");
	CHECK(sp_grow(&out, &again) == -1);
	out = g;
	out.part[3].len -= 1; /* data short of the end */
	CHECK(sp_shrink(&out, &s, &again) == -1);
	out = g;
	out.part[2].len += 2048; /* rootfs other than the factory's */
	out.part[3].addr += 2048;
	out.part[3].len -= 2048;
	CHECK(sp_shrink(&out, &s, &again) == -1);

	if (failures) {
		fprintf(stderr, "%d failed\n", failures);
		return 1;
	}
	printf("sunxi-part layout tests: all passed\n");
	return 0;
}
