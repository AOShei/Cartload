/*
 * tinysvcmdns: announces the console's SSH service over mDNS, as hakchi's init script runs it:
 *
 *   tinysvcmdns <instance> <service type> <port> [txt...]
 *   tinysvcmdns hakchi _ssh._tcp 22 hwid=... type=... region=...
 *
 * hakchi v1.0.4-126 shipped a program of this name around geekman's tinysvcmdns library whose
 * main was never published; this one does the same with that library (mdns.c, mdnsd.c). The
 * host name is the system's, published as <hostname>.local with the address of the first
 * IPv4 interface that is up and not loopback (the USB network). SIGHUP reads the address
 * again (S93mdnsd's "reload"). BSD-3-Clause, like the library.
 */
#include <arpa/inet.h>
#include <ifaddrs.h>
#include <net/if.h>
#include <netinet/in.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "mdns.h"
#include "mdnsd.h"

static volatile sig_atomic_t reload;

static void on_hup(int sig) {
	(void)sig;
	reload = 1;
}

/* The first IPv4 address of an interface that is up and not loopback, network order; 0 if none. */
static uint32_t local_address(void) {
	struct ifaddrs *list, *i;
	uint32_t ip = 0;
	if (getifaddrs(&list) != 0)
		return 0;
	for (i = list; i != NULL && ip == 0; i = i->ifa_next) {
		if (i->ifa_addr == NULL || i->ifa_addr->sa_family != AF_INET)
			continue;
		if (!(i->ifa_flags & IFF_UP) || (i->ifa_flags & IFF_LOOPBACK))
			continue;
		ip = ((struct sockaddr_in *)i->ifa_addr)->sin_addr.s_addr;
	}
	freeifaddrs(list);
	return ip;
}

int main(int argc, char *argv[]) {
	char host[256], fqdn[300], type[300];
	struct in_addr shown;
	struct mdnsd *svr;
	int port;

	if (argc < 4) {
		fprintf(stderr, "usage: %s <instance> <service type> <port> [txt...]\n", argv[0]);
		return 1;
	}
	port = atoi(argv[3]);
	if (port <= 0 || port > 65535) {
		fprintf(stderr, "invalid port: %s\n", argv[3]);
		return 1;
	}
	if (gethostname(host, sizeof(host)) != 0)
		strcpy(host, "hakchi");
	host[sizeof(host) - 1] = '\0';
	snprintf(fqdn, sizeof(fqdn), "%s.local", host);
	snprintf(type, sizeof(type), "%s.local", argv[2]);

	svr = mdnsd_start();
	if (svr == NULL) {
		fprintf(stderr, "mdnsd_start() failed\n");
		return 1;
	}
	shown.s_addr = local_address();
	mdnsd_set_hostname(svr, fqdn, shown.s_addr);
	printf("hostname: %s (%s)\n", fqdn, inet_ntoa(shown));

	/* argv[4..] are the TXT records; mdnsd_register_svc wants a NULL-terminated list. */
	mdns_service_destroy(mdnsd_register_svc(svr, argv[1], type, port, NULL,
						(const char **)&argv[4]));
	printf("service: %s %s %d\n", argv[1], type, port);

	signal(SIGHUP, on_hup);
	for (;;) {
		pause();
		if (reload) {
			reload = 0;
			shown.s_addr = local_address();
			mdnsd_set_hostname(svr, fqdn, shown.s_addr);
			printf("hostname: %s (%s)\n", fqdn, inet_ntoa(shown));
		}
	}
}
