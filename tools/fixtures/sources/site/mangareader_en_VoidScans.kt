package org.zeravrix.parsers.site.mangareader.en

import org.zeravrix.parsers.Broken
import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.mangareader.MangaReaderParser

@Broken("Redirect to HiveComic")
@MangaSourceParser("VOIDSCANS", "HiveToon", "en")
internal class VoidScans(context: MangaLoaderContext) :
	MangaReaderParser(context, MangaParserSource.VOIDSCANS, "hivetoon.com", pageSize = 15, searchPageSize = 10)
